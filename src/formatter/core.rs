use rowan::{NodeOrToken, SyntaxElement, SyntaxToken};

use super::context::FormatContext;
use super::style::FormatStyle;
use crate::parser::parse;
use crate::syntax::{RLanguage, SyntaxKind, SyntaxNode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatError {
    ParseErrors {
        count: usize,
    },
    UnsupportedConstruct {
        kind: SyntaxKind,
        snippet: String,
    },
    AmbiguousConstruct {
        context: &'static str,
        snippet: String,
    },
}

impl std::fmt::Display for FormatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ParseErrors { count } => write!(
                f,
                "input contains {count} parser diagnostic(s); formatter only supports parseable input"
            ),
            Self::UnsupportedConstruct { kind, snippet } => {
                write!(
                    f,
                    "unsupported construct for formatter: {kind:?} near {snippet:?}"
                )
            }
            Self::AmbiguousConstruct { context, snippet } => {
                write!(
                    f,
                    "ambiguous construct for formatter ({context}): {snippet:?}"
                )
            }
        }
    }
}

impl std::error::Error for FormatError {}

pub fn format(input: &str) -> Result<String, FormatError> {
    format_with_style(input, FormatStyle::default())
}

pub fn format_with_style(input: &str, style: FormatStyle) -> Result<String, FormatError> {
    let parse_output = parse(input);
    if !parse_output.diagnostics.is_empty() {
        return Err(FormatError::ParseErrors {
            count: parse_output.diagnostics.len(),
        });
    }

    validate_supported_tokens(&parse_output.cst)?;
    let ctx = FormatContext::new(style);
    let mut formatted = format_root(&parse_output.cst, ctx)?;
    if input.ends_with('\n') && !formatted.ends_with('\n') {
        formatted.push('\n');
    }
    Ok(formatted)
}

fn validate_supported_tokens(root: &SyntaxNode) -> Result<(), FormatError> {
    for element in root.descendants_with_tokens() {
        let Some(token) = element.into_token() else {
            continue;
        };
        let kind = token.kind();
        if matches!(
            kind,
            SyntaxKind::USER_OP
                | SyntaxKind::LBRACK
                | SyntaxKind::RBRACK
                | SyntaxKind::LBRACK2
                | SyntaxKind::RBRACK2
                | SyntaxKind::COLON2
                | SyntaxKind::COLON3
                | SyntaxKind::DOLLAR
                | SyntaxKind::AT
                | SyntaxKind::ERROR
        ) {
            return Err(FormatError::UnsupportedConstruct {
                kind,
                snippet: token.text().to_string(),
            });
        }
    }
    Ok(())
}

fn format_root(root: &SyntaxNode, ctx: FormatContext) -> Result<String, FormatError> {
    let lines = split_lines(root.children_with_tokens().collect(), "root")?;
    if lines.is_empty() {
        return Ok(String::new());
    }

    let mut out = String::new();
    let mut idx = 0usize;
    while idx < lines.len() {
        if idx > 0 {
            out.push('\n');
            if should_insert_comment_for_gap(&lines, idx, 0, ctx)? {
                out.push('\n');
            }
        }
        if let Some((formatted, consumed)) = try_format_for_with_external_body(&lines, idx, 0, ctx)?
        {
            out.push_str(&formatted);
            idx += consumed + 1;
            continue;
        }

        out.push_str(&format_line(&lines[idx], 0, ctx)?);
        idx += 1;
    }
    Ok(out)
}

fn format_line(
    line: &[SyntaxElement<RLanguage>],
    indent: usize,
    ctx: FormatContext,
) -> Result<String, FormatError> {
    let significant: Vec<_> = line
        .iter()
        .filter(|el| !is_trivia(el.kind()))
        .cloned()
        .collect();
    if significant.is_empty() {
        return Ok(String::new());
    }

    if let [NodeOrToken::Token(token)] = significant.as_slice()
        && token.kind() == SyntaxKind::COMMENT
    {
        return Ok(format!("{}{}", ctx.indent_text(indent), token.text()));
    }

    if significant.len() == 2
        && matches!(
            significant.last(),
            Some(NodeOrToken::Token(token)) if token.kind() == SyntaxKind::COMMENT
        )
    {
        let expr = format_expr_element(&significant[0], indent, ctx)?;
        let comment = match &significant[1] {
            NodeOrToken::Token(token) => token.text(),
            NodeOrToken::Node(_) => unreachable!(),
        };
        return Ok(format!("{}{} {}", ctx.indent_text(indent), expr, comment));
    }

    let expr = format_expr_segment(&significant, "line expression", indent, ctx)?;
    Ok(format!("{}{}", ctx.indent_text(indent), expr))
}

fn format_expr_segment(
    elements: &[SyntaxElement<RLanguage>],
    context: &'static str,
    indent: usize,
    ctx: FormatContext,
) -> Result<String, FormatError> {
    let significant: Vec<_> = elements
        .iter()
        .filter(|el| !is_trivia(el.kind()))
        .cloned()
        .collect();
    if significant.len() != 1 {
        return Err(FormatError::AmbiguousConstruct {
            context,
            snippet: snippet_from_elements(elements),
        });
    }
    format_expr_element(&significant[0], indent, ctx)
}

fn format_expr_element(
    element: &SyntaxElement<RLanguage>,
    indent: usize,
    ctx: FormatContext,
) -> Result<String, FormatError> {
    match element {
        NodeOrToken::Node(node) => format_expr_node(node, indent, ctx),
        NodeOrToken::Token(token) => format_atom_token(token),
    }
}

fn format_expr_node(
    node: &SyntaxNode,
    indent: usize,
    ctx: FormatContext,
) -> Result<String, FormatError> {
    match node.kind() {
        SyntaxKind::ASSIGNMENT_EXPR => format_assignment_expr(node, indent, ctx),
        SyntaxKind::UNARY_EXPR => format_unary_expr(node, indent, ctx),
        SyntaxKind::BINARY_EXPR => format_binary_expr(node, indent, ctx),
        SyntaxKind::PAREN_EXPR => format_paren_expr(node, indent, ctx),
        SyntaxKind::CALL_EXPR => format_call_expr(node, indent, ctx),
        SyntaxKind::IF_EXPR => format_if_expr(node, indent, ctx),
        SyntaxKind::FOR_EXPR => format_for_expr(node, indent, ctx),
        SyntaxKind::BLOCK_EXPR => format_block_expr(node, indent, ctx),
        SyntaxKind::FUNCTION_EXPR => format_function_expr(node, indent, ctx),
        kind => Err(FormatError::UnsupportedConstruct {
            kind,
            snippet: node.text().to_string(),
        }),
    }
}

fn format_unary_expr(
    node: &SyntaxNode,
    indent: usize,
    ctx: FormatContext,
) -> Result<String, FormatError> {
    let elements: Vec<_> = node.children_with_tokens().collect();
    let op_idx = elements
        .iter()
        .position(|el| {
            matches!(
                el,
                NodeOrToken::Token(tok)
                    if matches!(tok.kind(), SyntaxKind::PLUS | SyntaxKind::MINUS | SyntaxKind::BANG)
            )
        })
        .ok_or_else(|| FormatError::AmbiguousConstruct {
            context: "unary operator not found",
            snippet: node.text().to_string(),
        })?;
    let op = match &elements[op_idx] {
        NodeOrToken::Token(tok) => tok.text().to_string(),
        NodeOrToken::Node(_) => unreachable!(),
    };
    let rhs = format_expr_segment(&elements[op_idx + 1..], "unary operand", indent, ctx)?;
    Ok(format!("{op}{rhs}"))
}

fn format_assignment_expr(
    node: &SyntaxNode,
    indent: usize,
    ctx: FormatContext,
) -> Result<String, FormatError> {
    let elements: Vec<_> = node.children_with_tokens().collect();
    let op_idx = elements
        .iter()
        .position(|el| {
            matches!(
                el,
                NodeOrToken::Token(tok)
                    if matches!(
                        tok.kind(),
                        SyntaxKind::ASSIGN_LEFT
                            | SyntaxKind::SUPER_ASSIGN
                            | SyntaxKind::ASSIGN_RIGHT
                            | SyntaxKind::SUPER_ASSIGN_RIGHT
                            | SyntaxKind::ASSIGN_EQ
                    )
            )
        })
        .ok_or_else(|| FormatError::AmbiguousConstruct {
            context: "assignment operator not found",
            snippet: node.text().to_string(),
        })?;

    let op = match &elements[op_idx] {
        NodeOrToken::Token(tok) => tok.text().to_string(),
        NodeOrToken::Node(_) => unreachable!(),
    };
    let lhs = format_expr_segment(&elements[..op_idx], "assignment lhs", indent, ctx)?;
    let rhs = format_expr_segment(&elements[op_idx + 1..], "assignment rhs", indent, ctx)?;
    Ok(format!("{lhs} {op} {rhs}"))
}

fn format_binary_expr(
    node: &SyntaxNode,
    indent: usize,
    ctx: FormatContext,
) -> Result<String, FormatError> {
    let elements: Vec<_> = node.children_with_tokens().collect();
    let op_idx = elements
        .iter()
        .position(|el| {
            matches!(
                el,
                NodeOrToken::Token(tok)
                    if matches!(
                        tok.kind(),
                        SyntaxKind::PLUS
                            | SyntaxKind::MINUS
                            | SyntaxKind::STAR
                            | SyntaxKind::SLASH
                            | SyntaxKind::CARET
                            | SyntaxKind::PIPE
                            | SyntaxKind::COLON
                            | SyntaxKind::OR
                            | SyntaxKind::OR2
                            | SyntaxKind::AND
                            | SyntaxKind::AND2
                            | SyntaxKind::EQUAL2
                            | SyntaxKind::NOT_EQUAL
                            | SyntaxKind::LESS_THAN
                            | SyntaxKind::LESS_THAN_OR_EQUAL
                            | SyntaxKind::GREATER_THAN
                            | SyntaxKind::GREATER_THAN_OR_EQUAL
                            | SyntaxKind::TILDE
                            | SyntaxKind::USER_OP
                    )
            )
        })
        .ok_or_else(|| FormatError::AmbiguousConstruct {
            context: "binary operator not found",
            snippet: node.text().to_string(),
        })?;

    let (op_kind, op_text) = match &elements[op_idx] {
        NodeOrToken::Token(tok) => (tok.kind(), tok.text().to_string()),
        NodeOrToken::Node(_) => unreachable!(),
    };
    let lhs = format_expr_segment(&elements[..op_idx], "binary lhs", indent, ctx)?;
    let rhs = format_expr_segment(&elements[op_idx + 1..], "binary rhs", indent, ctx)?;
    let (inline, multiline) = if op_kind == SyntaxKind::CARET || op_kind == SyntaxKind::COLON {
        (
            format!("{lhs}{op_text}{rhs}"),
            format!("{lhs}\n{}{}{rhs}", ctx.indent_text(indent + 1), op_text),
        )
    } else {
        (
            format!("{lhs} {op_text} {rhs}"),
            format!("{lhs}\n{}{} {rhs}", ctx.indent_text(indent + 1), op_text),
        )
    };
    if ctx.fits_inline(indent, &inline) {
        return Ok(inline);
    }

    Ok(multiline)
}

fn format_call_expr(
    node: &SyntaxNode,
    indent: usize,
    ctx: FormatContext,
) -> Result<String, FormatError> {
    let elements: Vec<_> = node.children_with_tokens().collect();
    let lparen_idx = elements
        .iter()
        .position(|el| matches!(el, NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::LPAREN))
        .ok_or_else(|| FormatError::AmbiguousConstruct {
            context: "missing '(' in call expression",
            snippet: node.text().to_string(),
        })?;

    let callee = format_expr_segment(&elements[..lparen_idx], "call callee", indent, ctx)?;

    let arg_list = elements
        .iter()
        .find_map(|el| match el {
            NodeOrToken::Node(n) if n.kind() == SyntaxKind::ARG_LIST => Some(n.clone()),
            _ => None,
        })
        .ok_or_else(|| FormatError::AmbiguousConstruct {
            context: "missing arg list in call expression",
            snippet: node.text().to_string(),
        })?;

    let formatted_args = format_arg_list(&arg_list, indent, ctx)?;
    let inline = format!("{callee}({formatted_args})");
    if ctx.fits_inline(indent, &inline) {
        return Ok(inline);
    }

    let multiline_args = format_arg_list_multiline(&arg_list, indent, ctx)?;
    Ok(format!(
        "{callee}(\n{multiline_args}\n{})",
        ctx.indent_text(indent)
    ))
}

fn format_arg_list(
    node: &SyntaxNode,
    indent: usize,
    ctx: FormatContext,
) -> Result<String, FormatError> {
    let args: Vec<_> = node
        .children()
        .filter(|n| n.kind() == SyntaxKind::ARG)
        .collect();
    let mut formatted = Vec::new();
    for arg in &args {
        let s = format_arg(arg, indent, ctx)?;
        if !s.is_empty() {
            formatted.push(s);
        }
    }
    Ok(formatted.join(", "))
}

fn format_arg_list_multiline(
    node: &SyntaxNode,
    indent: usize,
    ctx: FormatContext,
) -> Result<String, FormatError> {
    let args: Vec<_> = node
        .children()
        .filter(|n| n.kind() == SyntaxKind::ARG)
        .collect();

    let mut out = Vec::new();
    for (idx, arg) in args.iter().enumerate() {
        let formatted = format_arg(arg, indent + 1, ctx)?;
        if formatted.is_empty() {
            continue;
        }

        let mut line = format!("{}{}", ctx.indent_text(indent + 1), formatted);
        if idx + 1 < args.len() {
            line.push(',');
        }
        out.push(line);
    }

    Ok(out.join("\n"))
}

fn format_arg(node: &SyntaxNode, indent: usize, ctx: FormatContext) -> Result<String, FormatError> {
    let elements: Vec<_> = node.children_with_tokens().collect();
    if elements.is_empty() {
        return Ok(String::new());
    }
    if let Some(eq_idx) = elements
        .iter()
        .position(|el| matches!(el, NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::ASSIGN_EQ))
    {
        let name = format_expr_segment(&elements[..eq_idx], "named arg name", indent, ctx)?;
        let value = format_expr_with_optional_comment(
            &elements[eq_idx + 1..],
            "named arg value",
            indent,
            ctx,
        )?;
        return Ok(format!("{name} = {value}"));
    }
    format_expr_with_optional_comment(&elements, "positional arg", indent, ctx)
}

fn format_paren_expr(
    node: &SyntaxNode,
    indent: usize,
    ctx: FormatContext,
) -> Result<String, FormatError> {
    let elements: Vec<_> = node.children_with_tokens().collect();
    let open_idx = elements
        .iter()
        .position(|el| matches!(el, NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::LPAREN))
        .ok_or_else(|| FormatError::AmbiguousConstruct {
            context: "missing '(' in parenthesized expression",
            snippet: node.text().to_string(),
        })?;
    let close_idx = elements
        .iter()
        .rposition(|el| matches!(el, NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::RPAREN))
        .ok_or_else(|| FormatError::AmbiguousConstruct {
            context: "missing ')' in parenthesized expression",
            snippet: node.text().to_string(),
        })?;

    if close_idx <= open_idx {
        return Err(FormatError::AmbiguousConstruct {
            context: "invalid parenthesized expression bounds",
            snippet: node.text().to_string(),
        });
    }

    let inner = format_expr_segment(
        &elements[open_idx + 1..close_idx],
        "parenthesized expression",
        indent,
        ctx,
    )?;
    Ok(format!("({inner})"))
}

fn format_if_expr(
    node: &SyntaxNode,
    indent: usize,
    ctx: FormatContext,
) -> Result<String, FormatError> {
    let elements: Vec<_> = node.children_with_tokens().collect();
    let if_idx = elements
        .iter()
        .position(|el| matches!(el, NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::IF_KW))
        .ok_or_else(|| FormatError::AmbiguousConstruct {
            context: "missing 'if' keyword",
            snippet: node.text().to_string(),
        })?;
    let lparen_idx = elements
        .iter()
        .enumerate()
        .skip(if_idx + 1)
        .find_map(|(i, el)| match el {
            NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::LPAREN => Some(i),
            _ => None,
        })
        .ok_or_else(|| FormatError::AmbiguousConstruct {
            context: "missing '(' after if",
            snippet: node.text().to_string(),
        })?;
    let rparen_idx = elements
        .iter()
        .enumerate()
        .skip(lparen_idx + 1)
        .find_map(|(i, el)| match el {
            NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::RPAREN => Some(i),
            _ => None,
        })
        .ok_or_else(|| FormatError::AmbiguousConstruct {
            context: "missing ')' after if condition",
            snippet: node.text().to_string(),
        })?;

    let condition = format_expr_segment(
        &elements[lparen_idx + 1..rparen_idx],
        "if condition",
        indent,
        ctx,
    )?;

    let else_idx = elements
        .iter()
        .enumerate()
        .skip(rparen_idx + 1)
        .find_map(|(i, el)| match el {
            NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::ELSE_KW => Some(i),
            _ => None,
        });

    let then_range_end = else_idx.unwrap_or(elements.len());
    let then_expr = format_expr_segment(
        &elements[rparen_idx + 1..then_range_end],
        "if then branch",
        indent,
        ctx,
    )?;

    let mut out = format!("if ({condition}) {then_expr}");
    if let Some(else_idx) = else_idx {
        let else_expr =
            format_expr_segment(&elements[else_idx + 1..], "if else branch", indent, ctx)?;
        out.push_str(" else ");
        out.push_str(&else_expr);
    }
    Ok(out)
}

#[derive(Clone)]
struct ForExprParts {
    leading_comments: Vec<String>,
    post_clause_comments: Vec<String>,
    header: String,
    body: Option<SyntaxElement<RLanguage>>,
}

fn format_for_expr(
    node: &SyntaxNode,
    indent: usize,
    ctx: FormatContext,
) -> Result<String, FormatError> {
    let parts = parse_for_expr_parts(node, indent, ctx)?;
    let body = format_for_body(
        parts.body.as_ref(),
        indent,
        ctx,
        &parts.post_clause_comments,
    )?;

    let mut out = String::new();
    for comment in &parts.leading_comments {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&ctx.indent_text(indent));
        out.push_str(comment);
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str(&parts.header);
    out.push(' ');
    out.push_str(&body);
    Ok(out)
}

fn format_block_expr(
    node: &SyntaxNode,
    indent: usize,
    ctx: FormatContext,
) -> Result<String, FormatError> {
    format_block_expr_with_prefixed_comments(node, indent, ctx, &[])
}

fn format_block_expr_with_prefixed_comments(
    node: &SyntaxNode,
    indent: usize,
    ctx: FormatContext,
    prefixed_comments: &[String],
) -> Result<String, FormatError> {
    let elements: Vec<_> = node.children_with_tokens().collect();
    let open_idx = elements
        .iter()
        .position(|el| matches!(el, NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::LBRACE))
        .ok_or_else(|| FormatError::AmbiguousConstruct {
            context: "missing '{' in block",
            snippet: node.text().to_string(),
        })?;
    let close_idx = elements
        .iter()
        .rposition(|el| matches!(el, NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::RBRACE))
        .ok_or_else(|| FormatError::AmbiguousConstruct {
            context: "missing '}' in block",
            snippet: node.text().to_string(),
        })?;
    if close_idx <= open_idx {
        return Err(FormatError::AmbiguousConstruct {
            context: "invalid block bounds",
            snippet: node.text().to_string(),
        });
    }

    let lines = split_lines(elements[open_idx + 1..close_idx].to_vec(), "block body")?;
    if lines.is_empty() && prefixed_comments.is_empty() {
        return Ok("{}".to_string());
    }

    let mut out = String::from("{\n");
    let mut emitted_any = false;
    for comment in prefixed_comments {
        if emitted_any {
            out.push('\n');
        }
        out.push_str(&ctx.indent_text(indent + 1));
        out.push_str(comment);
        emitted_any = true;
    }
    for line in &lines {
        if emitted_any {
            out.push('\n');
        }
        out.push_str(&format_line(line, indent + 1, ctx)?);
        emitted_any = true;
    }
    out.push('\n');
    out.push_str(&ctx.indent_text(indent));
    out.push('}');
    Ok(out)
}

fn format_function_expr(
    node: &SyntaxNode,
    indent: usize,
    ctx: FormatContext,
) -> Result<String, FormatError> {
    let elements: Vec<_> = node.children_with_tokens().collect();
    let fn_idx = elements
        .iter()
        .position(|el| {
            matches!(
                el,
                NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::FUNCTION_KW
            )
        })
        .ok_or_else(|| FormatError::AmbiguousConstruct {
            context: "missing 'function' keyword",
            snippet: node.text().to_string(),
        })?;
    let lparen_idx = elements
        .iter()
        .enumerate()
        .skip(fn_idx + 1)
        .find_map(|(i, el)| match el {
            NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::LPAREN => Some(i),
            _ => None,
        })
        .ok_or_else(|| FormatError::AmbiguousConstruct {
            context: "missing '(' in function expression",
            snippet: node.text().to_string(),
        })?;

    let mut depth = 0;
    let rparen_idx = elements
        .iter()
        .enumerate()
        .skip(lparen_idx)
        .find_map(|(i, el)| match el {
            NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::LPAREN => {
                depth += 1;
                None
            }
            NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::RPAREN => {
                depth -= 1;
                if depth == 0 { Some(i) } else { None }
            }
            _ => None,
        })
        .ok_or_else(|| FormatError::AmbiguousConstruct {
            context: "missing ')' in function expression",
            snippet: node.text().to_string(),
        })?;

    let params = format_function_parameters(&elements[lparen_idx + 1..rparen_idx], indent, ctx)?;
    let body = format_expr_segment(&elements[rparen_idx + 1..], "function body", indent, ctx)?;
    Ok(format!("function({params}) {body}"))
}

fn format_function_parameters(
    elements: &[SyntaxElement<RLanguage>],
    indent: usize,
    ctx: FormatContext,
) -> Result<String, FormatError> {
    let significant: Vec<_> = elements
        .iter()
        .filter(|el| !is_trivia(el.kind()))
        .cloned()
        .collect();
    if significant.is_empty() {
        return Ok(String::new());
    }

    let mut params: Vec<Vec<SyntaxElement<RLanguage>>> = Vec::new();
    let mut current: Vec<SyntaxElement<RLanguage>> = Vec::new();
    let mut depth = 0usize;

    for element in significant {
        match &element {
            NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::COMMA && depth == 0 => {
                if current.is_empty() {
                    return Err(FormatError::AmbiguousConstruct {
                        context: "empty function parameter",
                        snippet: tok.text().to_string(),
                    });
                }
                params.push(std::mem::take(&mut current));
            }
            NodeOrToken::Token(tok)
                if matches!(
                    tok.kind(),
                    SyntaxKind::LPAREN
                        | SyntaxKind::LBRACE
                        | SyntaxKind::LBRACK
                        | SyntaxKind::LBRACK2
                ) =>
            {
                depth += 1;
                current.push(element);
            }
            NodeOrToken::Token(tok)
                if matches!(
                    tok.kind(),
                    SyntaxKind::RPAREN
                        | SyntaxKind::RBRACE
                        | SyntaxKind::RBRACK
                        | SyntaxKind::RBRACK2
                ) =>
            {
                depth = depth.saturating_sub(1);
                current.push(element);
            }
            _ => current.push(element),
        }
    }

    if current.is_empty() {
        return Err(FormatError::AmbiguousConstruct {
            context: "trailing comma in function parameters",
            snippet: snippet_from_elements(elements),
        });
    }
    params.push(current);

    let mut out = Vec::with_capacity(params.len());
    for param in params {
        out.push(format_expr_segment(
            &param,
            "function parameter",
            indent,
            ctx,
        )?);
    }
    Ok(out.join(", "))
}

fn format_atom_token(token: &SyntaxToken<RLanguage>) -> Result<String, FormatError> {
    match token.kind() {
        SyntaxKind::IDENT
        | SyntaxKind::INT
        | SyntaxKind::FLOAT
        | SyntaxKind::STRING
        | SyntaxKind::BANG => Ok(token.text().to_string()),
        kind => Err(FormatError::UnsupportedConstruct {
            kind,
            snippet: token.text().to_string(),
        }),
    }
}

fn split_lines(
    elements: Vec<SyntaxElement<RLanguage>>,
    context: &'static str,
) -> Result<Vec<Vec<SyntaxElement<RLanguage>>>, FormatError> {
    let mut lines: Vec<Vec<SyntaxElement<RLanguage>>> = Vec::new();
    let mut current: Vec<SyntaxElement<RLanguage>> = Vec::new();
    let mut break_count = 0usize;

    for element in elements {
        if let NodeOrToken::Token(token) = &element {
            if token.kind() == SyntaxKind::WHITESPACE {
                continue;
            }
            if token.kind() == SyntaxKind::NEWLINE || token.kind() == SyntaxKind::SEMICOLON {
                if !current.is_empty() {
                    lines.push(std::mem::take(&mut current));
                    break_count = 1;
                } else if !lines.is_empty() {
                    break_count += 1;
                }
                continue;
            }
        }

        if break_count >= 2
            && (!matches!(lines.last(), Some(last) if is_comment_only_line(last))
                || matches!(element, NodeOrToken::Token(ref tok) if tok.kind() == SyntaxKind::COMMENT))
        {
            lines.push(Vec::new());
        }
        break_count = 0;

        if !current.is_empty() {
            if matches!(element, NodeOrToken::Token(ref tok) if tok.kind() == SyntaxKind::COMMENT)
                && !current.iter().any(
                    |el| matches!(el, NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::COMMENT),
                )
            {
                current.push(element);
                continue;
            }
            return Err(FormatError::AmbiguousConstruct {
                context,
                snippet: snippet_from_elements(&[current[0].clone(), element]),
            });
        }
        current.push(element);
    }

    if !current.is_empty() {
        lines.push(current);
    }

    Ok(lines)
}

fn snippet_from_elements(elements: &[SyntaxElement<RLanguage>]) -> String {
    elements
        .iter()
        .map(|el| match el {
            NodeOrToken::Node(node) => node.text().to_string(),
            NodeOrToken::Token(tok) => tok.text().to_string(),
        })
        .collect::<String>()
}

fn format_expr_with_optional_comment(
    elements: &[SyntaxElement<RLanguage>],
    context: &'static str,
    indent: usize,
    ctx: FormatContext,
) -> Result<String, FormatError> {
    let significant: Vec<_> = elements
        .iter()
        .filter(|el| !is_trivia(el.kind()))
        .cloned()
        .collect();

    if significant.len() == 2
        && matches!(
            significant.last(),
            Some(NodeOrToken::Token(token)) if token.kind() == SyntaxKind::COMMENT
        )
    {
        let expr = format_expr_element(&significant[0], indent, ctx)?;
        let comment = match &significant[1] {
            NodeOrToken::Token(token) => token.text(),
            NodeOrToken::Node(_) => unreachable!(),
        };
        return Ok(format!("{expr} {comment}"));
    }

    format_expr_segment(elements, context, indent, ctx)
}

fn parse_for_expr_parts(
    node: &SyntaxNode,
    indent: usize,
    ctx: FormatContext,
) -> Result<ForExprParts, FormatError> {
    let elements: Vec<_> = node.children_with_tokens().collect();
    let for_idx = elements
        .iter()
        .position(|el| matches!(el, NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::FOR_KW))
        .ok_or_else(|| FormatError::AmbiguousConstruct {
            context: "missing 'for' keyword",
            snippet: node.text().to_string(),
        })?;
    let lparen_idx = elements
        .iter()
        .enumerate()
        .skip(for_idx + 1)
        .find_map(|(i, el)| match el {
            NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::LPAREN => Some(i),
            _ => None,
        })
        .ok_or_else(|| FormatError::AmbiguousConstruct {
            context: "missing '(' after for",
            snippet: node.text().to_string(),
        })?;

    let mut depth = 0usize;
    let rparen_idx = elements
        .iter()
        .enumerate()
        .skip(lparen_idx)
        .find_map(|(i, el)| match el {
            NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::LPAREN => {
                depth += 1;
                None
            }
            NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::RPAREN => {
                depth = depth.saturating_sub(1);
                if depth == 0 { Some(i) } else { None }
            }
            _ => None,
        })
        .ok_or_else(|| FormatError::AmbiguousConstruct {
            context: "missing ')' after for clause",
            snippet: node.text().to_string(),
        })?;

    let leading_comments: Vec<_> = elements[for_idx + 1..rparen_idx]
        .iter()
        .filter_map(|el| match el {
            NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::COMMENT => {
                Some(tok.text().to_string())
            }
            _ => None,
        })
        .collect();
    let clause_elements: Vec<_> = elements[lparen_idx + 1..rparen_idx]
        .iter()
        .filter(|el| !is_trivia(el.kind()) && el.kind() != SyntaxKind::COMMENT)
        .cloned()
        .collect();
    let in_idx = clause_elements
        .iter()
        .position(|el| matches!(el, NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::IN_KW))
        .ok_or_else(|| FormatError::AmbiguousConstruct {
            context: "missing 'in' in for clause",
            snippet: node.text().to_string(),
        })?;
    let variable =
        format_expr_segment(&clause_elements[..in_idx], "for loop variable", indent, ctx)?;
    let sequence = format_expr_segment(
        &clause_elements[in_idx + 1..],
        "for loop sequence",
        indent,
        ctx,
    )?;

    let trailing = &elements[rparen_idx + 1..];
    let mut post_clause_comments = Vec::new();
    let mut body = None;
    for element in trailing {
        if is_trivia(element.kind()) {
            continue;
        }
        if let NodeOrToken::Token(tok) = element
            && tok.kind() == SyntaxKind::COMMENT
        {
            post_clause_comments.push(tok.text().to_string());
            continue;
        }
        body = Some(element.clone());
        break;
    }

    Ok(ForExprParts {
        leading_comments,
        post_clause_comments,
        header: format!("for ({variable} in {sequence})"),
        body,
    })
}

fn format_for_body(
    body: Option<&SyntaxElement<RLanguage>>,
    indent: usize,
    ctx: FormatContext,
    prefixed_comments: &[String],
) -> Result<String, FormatError> {
    match body {
        Some(NodeOrToken::Node(node)) if node.kind() == SyntaxKind::BLOCK_EXPR => {
            format_block_expr_with_prefixed_comments(node, indent, ctx, prefixed_comments)
        }
        Some(element) => {
            let expr = format_expr_element(element, indent + 1, ctx)?;
            let mut out = String::from("{\n");
            for comment in prefixed_comments {
                out.push_str(&ctx.indent_text(indent + 1));
                out.push_str(comment);
                out.push('\n');
            }
            out.push_str(&ctx.indent_text(indent + 1));
            out.push_str(&expr);
            out.push('\n');
            out.push_str(&ctx.indent_text(indent));
            out.push('}');
            Ok(out)
        }
        None => {
            if prefixed_comments.is_empty() {
                return Ok("{}".to_string());
            }
            let mut out = String::from("{\n");
            for (idx, comment) in prefixed_comments.iter().enumerate() {
                out.push_str(&ctx.indent_text(indent + 1));
                out.push_str(comment);
                if idx + 1 < prefixed_comments.len() {
                    out.push('\n');
                }
            }
            out.push('\n');
            out.push_str(&ctx.indent_text(indent));
            out.push('}');
            Ok(out)
        }
    }
}

fn significant_elements(line: &[SyntaxElement<RLanguage>]) -> Vec<SyntaxElement<RLanguage>> {
    line.iter()
        .filter(|el| !is_trivia(el.kind()))
        .cloned()
        .collect()
}

fn comment_only_line_text(line: &[SyntaxElement<RLanguage>]) -> Option<String> {
    match significant_elements(line).as_slice() {
        [NodeOrToken::Token(tok)] if tok.kind() == SyntaxKind::COMMENT => {
            Some(tok.text().to_string())
        }
        _ => None,
    }
}

fn try_format_for_with_external_body(
    lines: &[Vec<SyntaxElement<RLanguage>>],
    line_idx: usize,
    indent: usize,
    ctx: FormatContext,
) -> Result<Option<(String, usize)>, FormatError> {
    let significant = significant_elements(&lines[line_idx]);
    let (for_node, trailing_comment) = match significant.as_slice() {
        [NodeOrToken::Node(node)] if node.kind() == SyntaxKind::FOR_EXPR => (node.clone(), None),
        [NodeOrToken::Node(node), NodeOrToken::Token(tok)]
            if node.kind() == SyntaxKind::FOR_EXPR && tok.kind() == SyntaxKind::COMMENT =>
        {
            (node.clone(), Some(tok.text().to_string()))
        }
        _ => return Ok(None),
    };

    let parts = parse_for_expr_parts(&for_node, indent, ctx)?;
    if parts.body.is_some() {
        return Ok(None);
    }

    let mut extra_body_comments = Vec::new();
    let mut cursor = line_idx + 1;
    while cursor < lines.len() {
        if let Some(comment) = comment_only_line_text(&lines[cursor]) {
            extra_body_comments.push(comment);
            cursor += 1;
            continue;
        }
        break;
    }

    let body_node = if cursor < lines.len() {
        match significant_elements(&lines[cursor]).as_slice() {
            [NodeOrToken::Node(node)] if node.kind() == SyntaxKind::BLOCK_EXPR => {
                Some(node.clone())
            }
            _ => None,
        }
    } else {
        None
    };
    let Some(body_node) = body_node else {
        return Ok(None);
    };

    let mut merged_comments = parts.post_clause_comments.clone();
    merged_comments.extend(extra_body_comments);
    let body = format_for_body(
        Some(&NodeOrToken::Node(body_node)),
        indent,
        ctx,
        &merged_comments,
    )?;

    let mut out = String::new();
    for comment in &parts.leading_comments {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&ctx.indent_text(indent));
        out.push_str(comment);
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str(&parts.header);
    out.push(' ');
    out.push_str(&body);
    if let Some(comment) = trailing_comment {
        out.push(' ');
        out.push_str(&comment);
    }

    Ok(Some((out, cursor - line_idx)))
}

fn is_trivia(kind: SyntaxKind) -> bool {
    matches!(kind, SyntaxKind::WHITESPACE | SyntaxKind::NEWLINE)
}

fn is_comment_only_line(line: &[SyntaxElement<RLanguage>]) -> bool {
    let significant: Vec<_> = line.iter().filter(|el| !is_trivia(el.kind())).collect();
    matches!(
        significant.as_slice(),
        [NodeOrToken::Token(tok)] if tok.kind() == SyntaxKind::COMMENT
    )
}

fn line_starts_with_for_expr(line: &[SyntaxElement<RLanguage>]) -> bool {
    let significant = significant_elements(line);
    match significant.as_slice() {
        [NodeOrToken::Node(node)] => node.kind() == SyntaxKind::FOR_EXPR,
        [NodeOrToken::Node(node), NodeOrToken::Token(tok)] => {
            node.kind() == SyntaxKind::FOR_EXPR && tok.kind() == SyntaxKind::COMMENT
        }
        _ => false,
    }
}

fn for_leading_comment_count(
    line: &[SyntaxElement<RLanguage>],
    indent: usize,
    ctx: FormatContext,
) -> Result<Option<usize>, FormatError> {
    let significant = significant_elements(line);
    let node = match significant.as_slice() {
        [NodeOrToken::Node(node)] if node.kind() == SyntaxKind::FOR_EXPR => Some(node.clone()),
        [NodeOrToken::Node(node), NodeOrToken::Token(tok)]
            if node.kind() == SyntaxKind::FOR_EXPR && tok.kind() == SyntaxKind::COMMENT =>
        {
            Some(node.clone())
        }
        _ => None,
    };
    let Some(node) = node else {
        return Ok(None);
    };

    Ok(Some(
        parse_for_expr_parts(&node, indent, ctx)?
            .leading_comments
            .len(),
    ))
}

fn should_insert_comment_for_gap(
    lines: &[Vec<SyntaxElement<RLanguage>>],
    idx: usize,
    indent: usize,
    ctx: FormatContext,
) -> Result<bool, FormatError> {
    if idx < 2 || !is_comment_only_line(&lines[idx - 1]) || !is_comment_only_line(&lines[idx - 2]) {
        return Ok(false);
    }
    if idx >= 3 && is_comment_only_line(&lines[idx - 3]) {
        return Ok(false);
    }
    if !line_starts_with_for_expr(&lines[idx]) {
        return Ok(false);
    }
    Ok(for_leading_comment_count(&lines[idx], indent, ctx)?.unwrap_or(0) <= 1)
}

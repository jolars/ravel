use rowan::{NodeOrToken, SyntaxElement};

use super::super::context::FormatContext;
use super::super::core::{
    FormatError, format_expr_segment, format_expr_with_optional_comment, snippet_from_elements,
};
use crate::syntax::{RLanguage, SyntaxKind, SyntaxNode};

pub(crate) fn format_call_expr(
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
    let parts = collect_call_arg_parts(node, indent, ctx)?;
    if parts.formatted_args.is_empty() {
        return Ok(String::new());
    }

    if !parts.has_non_empty_arg {
        return Ok(",".repeat(parts.comma_count));
    }

    let first_non_empty = parts
        .formatted_args
        .iter()
        .position(|arg| !arg.is_empty())
        .ok_or_else(|| FormatError::AmbiguousConstruct {
            context: "missing non-empty call argument",
            snippet: node.text().to_string(),
        })?;

    let mut out = String::new();
    for (idx, arg) in parts.formatted_args.iter().enumerate() {
        out.push_str(arg);
        if idx < parts.comma_count {
            if idx + 1 < first_non_empty {
                out.push(',');
            } else {
                out.push_str(", ");
            }
        }
    }
    Ok(out)
}

fn format_arg_list_multiline(
    node: &SyntaxNode,
    indent: usize,
    ctx: FormatContext,
) -> Result<String, FormatError> {
    let parts = collect_call_arg_parts(node, indent + 1, ctx)?;
    if parts.formatted_args.is_empty() {
        return Ok(String::new());
    }

    let mut out = Vec::new();
    for (idx, formatted) in parts.formatted_args.iter().enumerate() {
        let mut line = format!("{}{}", ctx.indent_text(indent + 1), formatted);
        if idx < parts.comma_count {
            line.push(',');
        }
        out.push(line);
    }

    Ok(out.join("\n"))
}

struct CallArgParts {
    formatted_args: Vec<String>,
    comma_count: usize,
    has_non_empty_arg: bool,
}

fn collect_call_arg_parts(
    node: &SyntaxNode,
    indent: usize,
    ctx: FormatContext,
) -> Result<CallArgParts, FormatError> {
    let mut formatted_args = Vec::new();
    let mut comma_count = 0usize;

    for element in node.children_with_tokens() {
        match element {
            NodeOrToken::Node(arg) if arg.kind() == SyntaxKind::ARG => {
                formatted_args.push(format_arg(&arg, indent, ctx)?);
            }
            NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::COMMA => {
                comma_count += 1;
            }
            _ => {}
        }
    }

    let has_non_empty_arg = formatted_args.iter().any(|arg| !arg.is_empty());
    Ok(CallArgParts {
        formatted_args,
        comma_count,
        has_non_empty_arg,
    })
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

pub(crate) fn format_function_expr(
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
        .filter(|el| !super::super::core::is_trivia(el.kind()))
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

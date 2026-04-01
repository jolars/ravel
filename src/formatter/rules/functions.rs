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

    let parts = collect_call_arg_parts(&arg_list, indent, ctx)?;
    let formatted_args = format_arg_list_from_parts(&parts, &arg_list)?;
    let inline = format!("{callee}({formatted_args})");
    if !parts.has_comment_arg && ctx.fits_inline(indent, &inline) {
        return Ok(inline);
    }

    let multiline_args = format_arg_list_multiline(&arg_list, indent, ctx)?;
    Ok(format!(
        "{callee}(\n{multiline_args}\n{})",
        ctx.indent_text(indent)
    ))
}

fn format_arg_list_from_parts(
    node_parts: &CallArgParts,
    node: &SyntaxNode,
) -> Result<String, FormatError> {
    if node_parts.formatted_args.is_empty() {
        return Ok(String::new());
    }

    if !node_parts.has_non_empty_arg {
        return Ok(",".repeat(node_parts.comma_count));
    }

    let first_non_empty = node_parts
        .formatted_args
        .iter()
        .position(|arg| !arg.is_empty())
        .ok_or_else(|| FormatError::AmbiguousConstruct {
            context: "missing non-empty call argument",
            snippet: node.text().to_string(),
        })?;

    let mut out = String::new();
    for (idx, arg) in node_parts.formatted_args.iter().enumerate() {
        out.push_str(arg);
        if idx < node_parts.comma_count {
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
    let items = collect_call_items(node, indent + 1, ctx)?;
    if items.is_empty() {
        return Ok(String::new());
    }

    let mut out = Vec::new();
    let item_indent = ctx.indent_text(indent + 1);
    let mut i = 0usize;
    while i < items.len() {
        match &items[i] {
            CallItem::Arg(arg) if arg.is_empty => {
                i += 1;
            }
            CallItem::Arg(arg) if arg.is_comment_only => {
                out.push(format!("{item_indent}{}", arg.formatted));
                i += 1;
            }
            CallItem::Arg(arg) => {
                if let (
                    Some(CallItem::Comma {
                        newline_after: false,
                    }),
                    Some(CallItem::Arg(comment_arg)),
                ) = (items.get(i + 1), items.get(i + 2))
                    && comment_arg.is_comment_only
                {
                    out.push(format!(
                        "{item_indent}{}, {}",
                        arg.formatted, comment_arg.formatted
                    ));
                    i += 3;
                    while let Some(CallItem::Arg(extra_comment)) = items.get(i) {
                        if !extra_comment.is_comment_only {
                            break;
                        }
                        out.push(format!("{item_indent}   {}", extra_comment.formatted));
                        i += 1;
                    }
                    continue;
                }

                if matches!(items.get(i + 1), Some(CallItem::Comma { .. })) {
                    out.push(format!("{item_indent}{},", arg.formatted));
                    i += 2;
                } else {
                    out.push(format!("{item_indent}{}", arg.formatted));
                    i += 1;
                }
            }
            CallItem::Comma { .. } => {
                out.push(format!("{item_indent},"));
                i += 1;
            }
        }
    }

    Ok(out.join("\n"))
}

enum CallItem {
    Arg(ArgInfo),
    Comma { newline_after: bool },
}

struct ArgInfo {
    formatted: String,
    is_empty: bool,
    is_comment_only: bool,
}

fn collect_call_items(
    node: &SyntaxNode,
    indent: usize,
    ctx: FormatContext,
) -> Result<Vec<CallItem>, FormatError> {
    let elements: Vec<_> = node.children_with_tokens().collect();
    let mut items = Vec::new();
    for (idx, element) in elements.iter().enumerate() {
        match element {
            NodeOrToken::Node(arg) if arg.kind() == SyntaxKind::ARG => {
                let formatted = format_arg(arg, indent, ctx)?;
                let is_empty = formatted.is_empty();
                let is_comment_only = is_comment_only_arg(arg);
                items.push(CallItem::Arg(ArgInfo {
                    formatted,
                    is_empty,
                    is_comment_only,
                }));
            }
            NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::COMMA => {
                let mut newline_after = false;
                for next in elements.iter().skip(idx + 1) {
                    match next {
                        NodeOrToken::Token(t) if t.kind() == SyntaxKind::NEWLINE => {
                            newline_after = true;
                        }
                        NodeOrToken::Token(t)
                            if t.kind() == SyntaxKind::WHITESPACE
                                || t.kind() == SyntaxKind::COMMENT => {}
                        NodeOrToken::Node(n) if n.kind() == SyntaxKind::ARG => break,
                        NodeOrToken::Token(t) if t.kind() == SyntaxKind::COMMA => break,
                        _ => break,
                    }
                }
                items.push(CallItem::Comma { newline_after });
            }
            _ => {}
        }
    }
    Ok(items)
}

struct CallArgParts {
    formatted_args: Vec<String>,
    comma_count: usize,
    has_non_empty_arg: bool,
    has_comment_arg: bool,
}

fn collect_call_arg_parts(
    node: &SyntaxNode,
    indent: usize,
    ctx: FormatContext,
) -> Result<CallArgParts, FormatError> {
    let mut formatted_args = Vec::new();
    let mut comment_arg_mask = Vec::new();
    let mut comma_count = 0usize;

    for element in node.children_with_tokens() {
        match element {
            NodeOrToken::Node(arg) if arg.kind() == SyntaxKind::ARG => {
                comment_arg_mask.push(is_comment_only_arg(&arg));
                formatted_args.push(format_arg(&arg, indent, ctx)?);
            }
            NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::COMMA => {
                comma_count += 1;
            }
            _ => {}
        }
    }

    let has_non_empty_arg = formatted_args.iter().any(|arg| !arg.is_empty());
    let has_comment_arg = comment_arg_mask.iter().any(|is_comment| *is_comment);
    Ok(CallArgParts {
        formatted_args,
        comma_count,
        has_non_empty_arg,
        has_comment_arg,
    })
}

fn is_comment_only_arg(node: &SyntaxNode) -> bool {
    let significant: Vec<_> = node
        .children_with_tokens()
        .filter(|el| !super::super::core::is_trivia(el.kind()))
        .collect();
    matches!(
        significant.as_slice(),
        [NodeOrToken::Token(tok)] if tok.kind() == SyntaxKind::COMMENT
    )
}

fn format_arg(node: &SyntaxNode, indent: usize, ctx: FormatContext) -> Result<String, FormatError> {
    let elements: Vec<_> = node.children_with_tokens().collect();
    if elements.is_empty() {
        return Ok(String::new());
    }
    let significant: Vec<_> = elements
        .iter()
        .filter(|el| !super::super::core::is_trivia(el.kind()))
        .cloned()
        .collect();
    if let [NodeOrToken::Token(tok)] = significant.as_slice()
        && tok.kind() == SyntaxKind::COMMENT
    {
        return Ok(tok.text().to_string());
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

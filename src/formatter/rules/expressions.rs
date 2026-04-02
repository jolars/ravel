use rowan::NodeOrToken;

use super::super::context::FormatContext;
use super::super::core::{FormatError, format_expr_segment};
use crate::syntax::{SyntaxKind, SyntaxNode};

pub(crate) fn format_unary_expr(
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

pub(crate) fn format_assignment_expr(
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

pub(crate) fn format_binary_expr(
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
                            | SyntaxKind::COLON2
                            | SyntaxKind::COLON3
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
    if op_kind == SyntaxKind::COLON2 || op_kind == SyntaxKind::COLON3 {
        return Ok(format!("{lhs}{op_text}{rhs}"));
    }
    let (inline, multiline) = if op_kind == SyntaxKind::CARET
        || op_kind == SyntaxKind::COLON
        || op_kind == SyntaxKind::COLON2
        || op_kind == SyntaxKind::COLON3
    {
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
    if op_kind == SyntaxKind::PIPE {
        return Ok(format!(
            "{lhs} {op_text}\n{}{}",
            ctx.indent_text(indent + 1),
            rhs
        ));
    }
    if ctx.fits_inline(indent, &inline) {
        return Ok(inline);
    }

    Ok(multiline)
}

pub(crate) fn format_paren_expr(
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

pub(crate) fn format_subset_expr(
    node: &SyntaxNode,
    indent: usize,
    ctx: FormatContext,
) -> Result<String, FormatError> {
    let elements: Vec<_> = node.children_with_tokens().collect();
    let (open_kind, close_kind) = match node.kind() {
        SyntaxKind::SUBSET_EXPR => (SyntaxKind::LBRACK, SyntaxKind::RBRACK),
        SyntaxKind::SUBSET2_EXPR => (SyntaxKind::LBRACK2, SyntaxKind::RBRACK2),
        _ => {
            return Err(FormatError::AmbiguousConstruct {
                context: "subset formatter called on non-subset node",
                snippet: node.text().to_string(),
            });
        }
    };
    let open_idx = elements
        .iter()
        .position(|el| matches!(el, NodeOrToken::Token(tok) if tok.kind() == open_kind))
        .ok_or_else(|| FormatError::AmbiguousConstruct {
            context: "missing opening bracket in subset expression",
            snippet: node.text().to_string(),
        })?;
    let close_idx = elements
        .iter()
        .rposition(|el| matches!(el, NodeOrToken::Token(tok) if tok.kind() == close_kind))
        .ok_or_else(|| FormatError::AmbiguousConstruct {
            context: "missing closing bracket in subset expression",
            snippet: node.text().to_string(),
        })?;
    if close_idx <= open_idx {
        return Err(FormatError::AmbiguousConstruct {
            context: "invalid subset bounds",
            snippet: node.text().to_string(),
        });
    }

    let target = format_expr_segment(&elements[..open_idx], "subset target", indent, ctx)?;
    let arg_list = elements
        .iter()
        .find_map(|el| match el {
            NodeOrToken::Node(n) if n.kind() == SyntaxKind::ARG_LIST => Some(n.clone()),
            _ => None,
        })
        .ok_or_else(|| FormatError::AmbiguousConstruct {
            context: "missing arg list in subset expression",
            snippet: node.text().to_string(),
        })?;

    let args = format_subset_args_inline(&arg_list, indent, ctx)?;
    let open = match open_kind {
        SyntaxKind::LBRACK => "[",
        SyntaxKind::LBRACK2 => "[[",
        _ => unreachable!(),
    };
    let close = match close_kind {
        SyntaxKind::RBRACK => "]",
        SyntaxKind::RBRACK2 => "]]",
        _ => unreachable!(),
    };
    Ok(format!("{target}{open}{args}{close}"))
}

fn format_subset_args_inline(
    arg_list: &SyntaxNode,
    indent: usize,
    ctx: FormatContext,
) -> Result<String, FormatError> {
    let parts = collect_subset_arg_parts(arg_list, indent, ctx)?;
    if parts.args.is_empty() {
        return Ok(String::new());
    }

    let inline_args = format_subset_args_inline_from_parts(&parts);
    let leading_hole = parts.commas > 0 && parts.args.first().is_some_and(|arg| arg.is_empty());
    let has_multiline_arg = parts.args.iter().any(|arg| arg.contains('\n'));
    let multiline_trailing_arg_allowed = leading_hole && can_inline_trailing_multiline_arg(&parts);
    let force_multiline = has_multiline_arg && !multiline_trailing_arg_allowed;
    if !force_multiline {
        return Ok(inline_args);
    }
    format_subset_args_multiline(&parts, indent, ctx)
}

struct SubsetArgParts {
    args: Vec<String>,
    commas: usize,
}

fn collect_subset_arg_parts(
    arg_list: &SyntaxNode,
    indent: usize,
    ctx: FormatContext,
) -> Result<SubsetArgParts, FormatError> {
    let mut args = Vec::new();
    let mut commas = 0usize;
    for element in arg_list.children_with_tokens() {
        match element {
            NodeOrToken::Node(arg) if arg.kind() == SyntaxKind::ARG => {
                let arg_elements: Vec<_> = arg.children_with_tokens().collect();
                if arg_elements.is_empty() {
                    args.push(String::new());
                } else {
                    args.push(format_expr_segment(
                        &arg_elements,
                        "subset argument",
                        indent,
                        ctx,
                    )?);
                }
            }
            NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::COMMA => commas += 1,
            _ => {}
        }
    }
    Ok(SubsetArgParts { args, commas })
}

fn format_subset_args_inline_from_parts(parts: &SubsetArgParts) -> String {
    let mut out = String::new();
    for (idx, arg) in parts.args.iter().enumerate() {
        out.push_str(arg);
        if idx < parts.commas {
            let next = parts.args.get(idx + 1).map_or("", String::as_str);
            if arg.is_empty() && next.is_empty() {
                out.push(',');
            } else if arg.is_empty() || next.is_empty() {
                if next.is_empty() {
                    out.push(',');
                } else {
                    out.push_str(", ");
                }
            } else {
                out.push_str(", ");
            }
        }
    }
    out
}

fn can_inline_trailing_multiline_arg(parts: &SubsetArgParts) -> bool {
    if parts.args.is_empty() {
        return false;
    }
    let last_idx = parts.args.len() - 1;
    if !parts.args[last_idx].contains('\n') {
        return false;
    }
    let non_empty_before_last = parts.args[..last_idx]
        .iter()
        .filter(|arg| !arg.is_empty())
        .count();
    non_empty_before_last <= 1
}

fn format_subset_args_multiline(
    parts: &SubsetArgParts,
    indent: usize,
    ctx: FormatContext,
) -> Result<String, FormatError> {
    let mut out = String::new();
    let item_indent = ctx.indent_text(indent + 1);
    let leading_hole = parts.commas > 0 && parts.args.first().is_some_and(|arg| arg.is_empty());
    let start_idx = if leading_hole { 1 } else { 0 };
    if !leading_hole {
        out.push('\n');
    }

    for idx in start_idx..parts.args.len() {
        let arg = &parts.args[idx];
        let has_trailing_comma = idx < parts.commas;
        if arg.is_empty() {
            out.push_str(&item_indent);
            if has_trailing_comma {
                out.push(',');
            }
            out.push('\n');
            continue;
        }
        let mut lines: Vec<String> = arg.lines().map(|line| line.to_string()).collect();
        if lines.is_empty() {
            continue;
        }
        lines[0] = format!("{item_indent}{}", lines[0]);
        for line in lines.iter_mut().skip(1) {
            *line = format!("{item_indent}{line}");
        }
        if has_trailing_comma && let Some(last) = lines.last_mut() {
            last.push(',');
        }
        out.push_str(&lines.join("\n"));
        out.push('\n');
    }

    out.push_str(&ctx.indent_text(indent));
    Ok(out)
}

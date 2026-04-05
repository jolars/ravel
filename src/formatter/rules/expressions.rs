use rowan::{NodeOrToken, SyntaxElement};

use super::super::context::FormatContext;
use super::super::core::{FormatError, format_expr_segment, format_expr_with_optional_comment};
use crate::syntax::{RLanguage, SyntaxKind, SyntaxNode};

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
                            | SyntaxKind::DOLLAR
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
        || op_kind == SyntaxKind::DOLLAR
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

    let args = format_subset_args(&arg_list, indent, ctx, &target, open_kind, close_kind)?;
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

fn format_subset_args(
    arg_list: &SyntaxNode,
    indent: usize,
    ctx: FormatContext,
    target: &str,
    open_kind: SyntaxKind,
    close_kind: SyntaxKind,
) -> Result<String, FormatError> {
    let parts = collect_subset_arg_parts(arg_list, indent, ctx)?;
    if parts.slots.is_empty() {
        return Ok(String::new());
    }

    let inline_args = format_subset_args_inline_from_parts(&parts, true);
    let inline_expr = format!(
        "{target}{}{}{}",
        bracket_open_text(open_kind),
        inline_args,
        bracket_close_text(close_kind)
    );
    let has_multiline_arg = parts.slots.iter().flatten().any(|arg| arg.contains('\n'));
    let force_multiline = parts.has_comment_only_slot
        || parts.has_comment_prefixed_expr_slot
        || should_force_subset_multiline(&parts)
        || !ctx.fits_with_newlines(indent, &inline_expr);
    if !force_multiline {
        return Ok(inline_args);
    }
    if parts.has_comment_only_slot {
        return format_subset_args_multiline_wrapped(&parts, indent, ctx);
    }
    format_subset_args_multiline(&parts, indent, ctx, has_multiline_arg)
}

struct SubsetArgParts {
    slots: Vec<Option<String>>,
    has_comment_only_slot: bool,
    has_comment_prefixed_expr_slot: bool,
}

fn collect_subset_arg_parts(
    arg_list: &SyntaxNode,
    indent: usize,
    ctx: FormatContext,
) -> Result<SubsetArgParts, FormatError> {
    let mut slots: Vec<Option<String>> = vec![None];
    let mut slot_idx = 0usize;
    let mut has_comment_only_slot = false;
    let mut has_comment_prefixed_expr_slot = false;
    for element in arg_list.children_with_tokens() {
        match element {
            NodeOrToken::Node(arg) if arg.kind() == SyntaxKind::ARG => {
                let arg_elements: Vec<_> = arg.children_with_tokens().collect();
                if arg_elements.is_empty() {
                    slots[slot_idx] = Some(String::new());
                } else {
                    let has_comment = arg_elements.iter().any(
                        |el| matches!(el, NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::COMMENT),
                    );
                    let has_non_comment = arg_elements.iter().any(|el| match el {
                        NodeOrToken::Node(_) => true,
                        NodeOrToken::Token(tok) => !matches!(
                            tok.kind(),
                            SyntaxKind::WHITESPACE | SyntaxKind::NEWLINE | SyntaxKind::COMMENT
                        ),
                    });
                    if has_comment && !has_non_comment {
                        let comment = arg_elements
                            .iter()
                            .find_map(|el| match el {
                                NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::COMMENT => {
                                    Some(tok.text().to_string())
                                }
                                _ => None,
                            })
                            .unwrap_or_default();
                        match slots[slot_idx].take() {
                            Some(existing) if !existing.is_empty() => {
                                slots[slot_idx] = Some(format!("{existing}\n{comment}"));
                            }
                            _ => {
                                slots[slot_idx] = Some(comment);
                            }
                        }
                        has_comment_only_slot = true;
                    } else {
                        let (formatted, has_comment_prefix) =
                            format_subset_argument(&arg_elements, indent, ctx)?;
                        match slots[slot_idx].take() {
                            Some(existing) if !existing.is_empty() && !formatted.is_empty() => {
                                slots[slot_idx] = Some(format!("{existing}\n{formatted}"));
                            }
                            Some(existing) if !existing.is_empty() => {
                                slots[slot_idx] = Some(existing);
                            }
                            _ => {
                                slots[slot_idx] = Some(formatted);
                            }
                        }
                        has_comment_prefixed_expr_slot |= has_comment_prefix;
                    }
                }
            }
            NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::COMMA => {
                slot_idx += 1;
                if slots.len() <= slot_idx {
                    slots.push(None);
                }
            }
            _ => {}
        }
    }
    Ok(SubsetArgParts {
        slots,
        has_comment_only_slot,
        has_comment_prefixed_expr_slot,
    })
}

fn format_subset_argument(
    elements: &[SyntaxElement<RLanguage>],
    indent: usize,
    ctx: FormatContext,
) -> Result<(String, bool), FormatError> {
    let mut first_non_comment_idx = None;
    for (idx, el) in elements.iter().enumerate() {
        if matches!(el, NodeOrToken::Token(tok) if matches!(tok.kind(), SyntaxKind::WHITESPACE | SyntaxKind::NEWLINE))
        {
            continue;
        }
        if matches!(el, NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::COMMENT) {
            continue;
        }
        first_non_comment_idx = Some(idx);
        break;
    }

    let Some(expr_start) = first_non_comment_idx else {
        return Ok((
            format_expr_segment(elements, "subset argument", indent, ctx)?,
            false,
        ));
    };

    let leading_comments = elements[..expr_start]
        .iter()
        .filter_map(|el| match el {
            NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::COMMENT => {
                Some(tok.text().to_string())
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    if leading_comments.is_empty() {
        return Ok((
            format_expr_segment(elements, "subset argument", indent, ctx)?,
            false,
        ));
    }

    let expr =
        format_expr_with_optional_comment(&elements[expr_start..], "subset argument", indent, ctx)?;
    Ok((format!("{}\n{expr}", leading_comments.join("\n")), true))
}

fn format_subset_args_inline_from_parts(
    parts: &SubsetArgParts,
    compact_before_first: bool,
) -> String {
    let first_non_empty = first_non_empty_slot(parts);
    let no_non_empty = first_non_empty.is_none();
    let mut out = String::new();
    for idx in 0..parts.slots.len() {
        if let Some(arg) = &parts.slots[idx] {
            out.push_str(arg);
        }
        if idx + 1 < parts.slots.len() {
            let left_empty = parts.slots[idx].as_deref().is_none_or(str::is_empty);
            let right_empty = parts.slots[idx + 1].as_deref().is_none_or(str::is_empty);
            if left_empty
                && right_empty
                && (no_non_empty
                    || first_non_empty.is_some_and(|first| idx + 1 < first && compact_before_first))
            {
                out.push(',');
            } else {
                out.push_str(", ");
            }
        }
    }
    out
}

fn should_force_subset_multiline(parts: &SubsetArgParts) -> bool {
    let Some(first_non_empty) = first_non_empty_slot(parts) else {
        return false;
    };
    let inline = format_subset_args_inline_from_parts(parts, true);
    let has_multiline = inline.contains('\n');
    if !has_multiline {
        return false;
    }
    let leading_hole = parts
        .slots
        .first()
        .is_some_and(|slot| slot.as_deref().is_none_or(str::is_empty));
    let non_empty_count = parts
        .slots
        .iter()
        .filter(|slot| slot.as_deref().is_some_and(|arg| !arg.is_empty()))
        .count();
    let first_is_multiline = parts.slots[first_non_empty]
        .as_deref()
        .is_some_and(|arg| arg.contains('\n'));
    leading_hole && first_is_multiline && non_empty_count > 1
}

fn first_non_empty_slot(parts: &SubsetArgParts) -> Option<usize> {
    parts
        .slots
        .iter()
        .position(|slot| slot.as_deref().is_some_and(|arg| !arg.is_empty()))
}

fn format_subset_args_multiline(
    parts: &SubsetArgParts,
    indent: usize,
    ctx: FormatContext,
    has_multiline_arg: bool,
) -> Result<String, FormatError> {
    let inline = format_subset_args_inline_from_parts(parts, true);
    if !has_multiline_arg {
        let candidate = format!("[{inline}]");
        if !ctx.fits_with_newlines(indent, &candidate) {
            return format_subset_args_multiline_wrapped(parts, indent, ctx);
        }
    }

    let mut out = String::new();
    let leading_hole = parts
        .slots
        .first()
        .is_some_and(|slot| slot.as_deref().is_none_or(str::is_empty));
    let hug_leading_hole = leading_hole && !parts.has_comment_prefixed_expr_slot;
    if hug_leading_hole {
        out.push(',');
    }
    out.push('\n');

    let first_non_empty = first_non_empty_slot(parts).unwrap_or(0);
    let item_indent = ctx.indent_text(indent + 1);
    for idx in (if hug_leading_hole { 1 } else { 0 })..parts.slots.len() {
        if idx > first_non_empty
            && parts.slots[idx]
                .as_deref()
                .is_some_and(|arg| arg.contains('\n'))
        {
            return format_subset_args_multiline_wrapped(parts, indent, ctx);
        }

        if let Some(arg) = parts.slots[idx].as_deref() {
            if arg.is_empty() {
                out.push_str(&item_indent);
                if idx + 1 < parts.slots.len() {
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
            if idx + 1 < parts.slots.len()
                && let Some(last) = lines.last_mut()
            {
                last.push(',');
            }
            out.push_str(&lines.join("\n"));
            out.push('\n');
            continue;
        }

        out.push_str(&item_indent);
        if idx + 1 < parts.slots.len() {
            out.push(',');
        }
        out.push('\n');
    }

    out.push_str(&ctx.indent_text(indent));
    Ok(out)
}

fn format_subset_args_multiline_wrapped(
    parts: &SubsetArgParts,
    indent: usize,
    ctx: FormatContext,
) -> Result<String, FormatError> {
    let mut out = String::from("\n");
    let item_indent = ctx.indent_text(indent + 1);
    for idx in 0..parts.slots.len() {
        let is_last = idx + 1 == parts.slots.len();
        let prev_is_empty = idx > 0 && parts.slots[idx - 1].as_deref().is_none_or(str::is_empty);
        if is_last && prev_is_empty && parts.slots[idx].as_deref().is_none_or(str::is_empty) {
            continue;
        }
        if let Some(arg) = parts.slots[idx].as_deref() {
            if arg.is_empty() {
                out.push_str(&item_indent);
                if idx + 1 < parts.slots.len() {
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
            if idx + 1 < parts.slots.len()
                && let Some(last) = lines.last_mut()
            {
                last.push(',');
            }
            out.push_str(&lines.join("\n"));
            out.push('\n');
            continue;
        }
        out.push_str(&item_indent);
        if idx + 1 < parts.slots.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(&ctx.indent_text(indent));
    Ok(out)
}

fn bracket_open_text(kind: SyntaxKind) -> &'static str {
    match kind {
        SyntaxKind::LBRACK => "[",
        SyntaxKind::LBRACK2 => "[[",
        _ => "",
    }
}

fn bracket_close_text(kind: SyntaxKind) -> &'static str {
    match kind {
        SyntaxKind::RBRACK => "]",
        SyntaxKind::RBRACK2 => "]]",
        _ => "",
    }
}

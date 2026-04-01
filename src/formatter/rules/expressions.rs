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

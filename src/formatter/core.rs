use rowan::{NodeOrToken, SyntaxElement};

use super::context::FormatContext;
use super::render::{format_atom_token, format_block_expr_with_prefixed_comments as render_block};
use super::rules::control_flow::{
    format_for_expr, format_if_expr, format_while_expr, should_insert_comment_for_gap,
    try_format_for_with_external_body, try_format_while_with_external_body,
};
use super::rules::expressions::{
    format_assignment_expr, format_binary_expr, format_paren_expr, format_unary_expr,
};
use super::rules::functions::{format_call_expr, format_function_expr};
use super::style::FormatStyle;
use super::trivia::{is_trivia as is_trivia_kind, split_lines};
use crate::ast::{
    AssignmentExpr, AstNode, BinaryExpr, BlockExpr, CallExpr, ForExpr, FunctionExpr, IfExpr,
    ParenExpr, UnaryExpr, WhileExpr,
};
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
        if let Some((formatted, consumed)) =
            try_format_while_with_external_body(&lines, idx, 0, ctx)?
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

pub(super) fn format_line(
    line: &[SyntaxElement<RLanguage>],
    indent: usize,
    ctx: FormatContext,
) -> Result<String, FormatError> {
    let significant: Vec<_> = line
        .iter()
        .filter(|el| !is_trivia_kind(el.kind()))
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

pub(super) fn format_expr_segment(
    elements: &[SyntaxElement<RLanguage>],
    context: &'static str,
    indent: usize,
    ctx: FormatContext,
) -> Result<String, FormatError> {
    super::render::format_expr_segment(elements, context, indent, ctx, format_expr_element)
}

pub(super) fn format_expr_element(
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
    if let Some(expr) = AssignmentExpr::cast(node.clone()) {
        return format_assignment_expr(expr.syntax(), indent, ctx);
    }
    if let Some(expr) = UnaryExpr::cast(node.clone()) {
        return format_unary_expr(expr.syntax(), indent, ctx);
    }
    if let Some(expr) = BinaryExpr::cast(node.clone()) {
        return format_binary_expr(expr.syntax(), indent, ctx);
    }
    if let Some(expr) = ParenExpr::cast(node.clone()) {
        return format_paren_expr(expr.syntax(), indent, ctx);
    }
    if let Some(expr) = CallExpr::cast(node.clone()) {
        return format_call_expr(expr.syntax(), indent, ctx);
    }
    if let Some(expr) = IfExpr::cast(node.clone()) {
        return format_if_expr(expr.syntax(), indent, ctx);
    }
    if let Some(expr) = ForExpr::cast(node.clone()) {
        return format_for_expr(expr.syntax(), indent, ctx);
    }
    if let Some(expr) = WhileExpr::cast(node.clone()) {
        return format_while_expr(expr.syntax(), indent, ctx);
    }
    if let Some(expr) = BlockExpr::cast(node.clone()) {
        return format_block_expr(expr.syntax(), indent, ctx);
    }
    if let Some(expr) = FunctionExpr::cast(node.clone()) {
        return format_function_expr(expr.syntax(), indent, ctx);
    }

    Err(FormatError::UnsupportedConstruct {
        kind: node.kind(),
        snippet: node.text().to_string(),
    })
}

fn format_block_expr(
    node: &SyntaxNode,
    indent: usize,
    ctx: FormatContext,
) -> Result<String, FormatError> {
    format_block_expr_with_prefixed_comments(node, indent, ctx, &[])
}

pub(super) fn format_block_expr_with_prefixed_comments(
    node: &SyntaxNode,
    indent: usize,
    ctx: FormatContext,
    prefixed_comments: &[String],
) -> Result<String, FormatError> {
    render_block(node, indent, ctx, prefixed_comments, format_line)
}

pub(super) fn snippet_from_elements(elements: &[SyntaxElement<RLanguage>]) -> String {
    super::render::snippet_from_elements(elements)
}

pub(super) fn format_expr_with_optional_comment(
    elements: &[SyntaxElement<RLanguage>],
    context: &'static str,
    indent: usize,
    ctx: FormatContext,
) -> Result<String, FormatError> {
    super::render::format_expr_with_optional_comment(
        elements,
        context,
        indent,
        ctx,
        format_expr_element,
    )
}

pub(super) fn is_trivia(kind: SyntaxKind) -> bool {
    is_trivia_kind(kind)
}

use rowan::{NodeOrToken, SyntaxElement};

use super::super::context::FormatContext;
use super::super::core::{
    FormatError, format_block_expr_with_prefixed_comments, format_expr_element,
    format_expr_segment, is_trivia,
};
use crate::ast::{AstNode, ForExpr, ForExprParts, IfExpr, WhileExpr, WhileExprParts};
use crate::syntax::{RLanguage, SyntaxKind, SyntaxNode};

pub(crate) fn format_if_expr(
    node: &SyntaxNode,
    indent: usize,
    ctx: FormatContext,
) -> Result<String, FormatError> {
    let if_expr = IfExpr::cast(node.clone()).ok_or_else(|| FormatError::AmbiguousConstruct {
        context: "invalid if expression node",
        snippet: node.text().to_string(),
    })?;

    if_expr
        .if_keyword()
        .ok_or_else(|| FormatError::AmbiguousConstruct {
            context: "missing 'if' keyword",
            snippet: node.text().to_string(),
        })?;
    if_expr
        .lparen_index()
        .ok_or_else(|| FormatError::AmbiguousConstruct {
            context: "missing '(' after if",
            snippet: node.text().to_string(),
        })?;
    if_expr
        .rparen_index()
        .ok_or_else(|| FormatError::AmbiguousConstruct {
            context: "missing ')' after if condition",
            snippet: node.text().to_string(),
        })?;

    let condition_elements =
        if_expr
            .condition_elements()
            .ok_or_else(|| FormatError::AmbiguousConstruct {
                context: "missing '(' after if",
                snippet: node.text().to_string(),
            })?;

    let then_elements = if_expr
        .then_elements()
        .ok_or_else(|| FormatError::AmbiguousConstruct {
            context: "missing ')' after if condition",
            snippet: node.text().to_string(),
        })?;

    let condition = format_expr_segment(&condition_elements, "if condition", indent, ctx)?;
    let then_expr = format_expr_segment(&then_elements, "if then branch", indent, ctx)?;

    let mut out = format!("if ({condition}) {then_expr}");
    if if_expr.else_keyword().is_some() {
        let else_elements =
            if_expr
                .else_elements()
                .ok_or_else(|| FormatError::AmbiguousConstruct {
                    context: "missing else branch",
                    snippet: node.text().to_string(),
                })?;
        let else_expr = format_expr_segment(&else_elements, "if else branch", indent, ctx)?;
        out.push_str(" else ");
        out.push_str(&else_expr);
    }
    Ok(out)
}

pub(crate) fn format_for_expr(
    node: &SyntaxNode,
    indent: usize,
    ctx: FormatContext,
) -> Result<String, FormatError> {
    let parts = parse_for_expr_parts(node, indent, ctx)?;
    let header = format_for_header(&parts, indent, ctx)?;
    let body = format_for_body(
        parts.body.as_ref(),
        indent,
        ctx,
        &parts
            .post_clause_comments
            .iter()
            .map(|tok| tok.text().to_string())
            .collect::<Vec<_>>(),
    )?;

    let mut out = String::new();
    for comment in &parts.leading_comments {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&ctx.indent_text(indent));
        out.push_str(comment.text());
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str(&header);
    out.push(' ');
    out.push_str(&body);
    Ok(out)
}

pub(crate) fn format_while_expr(
    node: &SyntaxNode,
    indent: usize,
    ctx: FormatContext,
) -> Result<String, FormatError> {
    let parts = parse_while_expr_parts(node, indent, ctx)?;
    let header = format_while_header(&parts, indent, ctx)?;
    let body = format_while_body(
        parts.body.as_ref(),
        indent,
        ctx,
        &parts
            .post_clause_comments
            .iter()
            .map(|tok| tok.text().to_string())
            .collect::<Vec<_>>(),
    )?;

    let mut out = String::new();
    for comment in &parts.leading_comments {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&ctx.indent_text(indent));
        out.push_str(comment.text());
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str(&header);
    out.push(' ');
    out.push_str(&body);
    Ok(out)
}

pub(crate) fn format_repeat_expr(
    node: &SyntaxNode,
    indent: usize,
    ctx: FormatContext,
) -> Result<String, FormatError> {
    let parts = parse_repeat_expr_parts(node)?;
    let body = format_repeat_body(
        parts.body.as_ref(),
        indent,
        ctx,
        &parts.post_keyword_comments,
    )?;
    Ok(format!("repeat {body}"))
}

pub(crate) fn try_format_for_with_external_body(
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

    let body_element = if cursor < lines.len() {
        match significant_elements(&lines[cursor]).as_slice() {
            [element] => Some(element.clone()),
            _ => None,
        }
    } else {
        None
    };
    let Some(body_element) = body_element else {
        return Ok(None);
    };

    let mut merged_comment_texts: Vec<String> = parts
        .post_clause_comments
        .iter()
        .map(|tok| tok.text().to_string())
        .collect();
    merged_comment_texts.extend(extra_body_comments);
    let body = format_for_body(Some(&body_element), indent, ctx, &merged_comment_texts)?;

    let mut out = String::new();
    for comment in &parts.leading_comments {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&ctx.indent_text(indent));
        out.push_str(comment.text());
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str(&format_for_header(&parts, indent, ctx)?);
    out.push(' ');
    out.push_str(&body);
    if let Some(comment) = trailing_comment {
        out.push(' ');
        out.push_str(&comment);
    }

    Ok(Some((out, cursor - line_idx)))
}

pub(crate) fn try_format_while_with_external_body(
    lines: &[Vec<SyntaxElement<RLanguage>>],
    line_idx: usize,
    indent: usize,
    ctx: FormatContext,
) -> Result<Option<(String, usize)>, FormatError> {
    let significant = significant_elements(&lines[line_idx]);
    let (while_node, trailing_comment) = match significant.as_slice() {
        [NodeOrToken::Node(node)] if node.kind() == SyntaxKind::WHILE_EXPR => (node.clone(), None),
        [NodeOrToken::Node(node), NodeOrToken::Token(tok)]
            if node.kind() == SyntaxKind::WHILE_EXPR && tok.kind() == SyntaxKind::COMMENT =>
        {
            (node.clone(), Some(tok.text().to_string()))
        }
        _ => return Ok(None),
    };

    let parts = parse_while_expr_parts(&while_node, indent, ctx)?;
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

    let body_element = if cursor < lines.len() {
        match significant_elements(&lines[cursor]).as_slice() {
            [element] => Some(element.clone()),
            _ => None,
        }
    } else {
        None
    };
    let Some(body_element) = body_element else {
        return Ok(None);
    };

    let mut merged_comment_texts: Vec<String> = parts
        .post_clause_comments
        .iter()
        .map(|tok| tok.text().to_string())
        .collect();
    merged_comment_texts.extend(extra_body_comments);
    let body = format_while_body(Some(&body_element), indent, ctx, &merged_comment_texts)?;

    let mut out = String::new();
    for comment in &parts.leading_comments {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&ctx.indent_text(indent));
        out.push_str(comment.text());
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str(&format_while_header(&parts, indent, ctx)?);
    out.push(' ');
    out.push_str(&body);
    if let Some(comment) = trailing_comment {
        out.push(' ');
        out.push_str(&comment);
    }

    Ok(Some((out, cursor - line_idx)))
}

pub(crate) fn try_format_repeat_with_external_body(
    lines: &[Vec<SyntaxElement<RLanguage>>],
    line_idx: usize,
    indent: usize,
    ctx: FormatContext,
) -> Result<Option<(String, usize)>, FormatError> {
    let significant = significant_elements(&lines[line_idx]);
    let (repeat_node, trailing_comment) = match significant.as_slice() {
        [NodeOrToken::Node(node)] if node.kind() == SyntaxKind::REPEAT_EXPR => (node.clone(), None),
        [NodeOrToken::Node(node), NodeOrToken::Token(tok)]
            if node.kind() == SyntaxKind::REPEAT_EXPR && tok.kind() == SyntaxKind::COMMENT =>
        {
            (node.clone(), Some(tok.text().to_string()))
        }
        _ => return Ok(None),
    };

    let parts = parse_repeat_expr_parts(&repeat_node)?;
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

    let body_element = if cursor < lines.len() {
        match significant_elements(&lines[cursor]).as_slice() {
            [element] => Some(element.clone()),
            _ => None,
        }
    } else {
        None
    };
    let Some(body_element) = body_element else {
        return Ok(None);
    };

    let mut merged_comments = parts.post_keyword_comments;
    merged_comments.extend(extra_body_comments);
    let body = format_repeat_body(Some(&body_element), indent, ctx, &merged_comments)?;

    let mut out = format!("repeat {body}");
    if let Some(comment) = trailing_comment {
        out.push(' ');
        out.push_str(&comment);
    }

    Ok(Some((out, cursor - line_idx)))
}

pub(crate) fn should_insert_comment_for_gap(
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
    if !line_starts_with_control_flow_loop(&lines[idx]) {
        return Ok(false);
    }
    Ok(loop_leading_comment_count(&lines[idx], indent, ctx)?.unwrap_or(0) <= 1)
}

fn parse_for_expr_parts(
    node: &SyntaxNode,
    _indent: usize,
    _ctx: FormatContext,
) -> Result<ForExprParts, FormatError> {
    let for_expr = ForExpr::cast(node.clone()).ok_or_else(|| FormatError::AmbiguousConstruct {
        context: "invalid for expression node",
        snippet: node.text().to_string(),
    })?;
    for_expr
        .parts()
        .ok_or_else(|| FormatError::AmbiguousConstruct {
            context: "invalid for expression structure",
            snippet: node.text().to_string(),
        })
}

fn format_for_header(
    parts: &ForExprParts,
    indent: usize,
    ctx: FormatContext,
) -> Result<String, FormatError> {
    let variable = format_expr_segment(&parts.variable_elements, "for loop variable", indent, ctx)?;
    let sequence = format_expr_segment(&parts.sequence_elements, "for loop sequence", indent, ctx)?;
    Ok(format!("for ({variable} in {sequence})"))
}

fn parse_while_expr_parts(
    node: &SyntaxNode,
    _indent: usize,
    _ctx: FormatContext,
) -> Result<WhileExprParts, FormatError> {
    let while_expr =
        WhileExpr::cast(node.clone()).ok_or_else(|| FormatError::AmbiguousConstruct {
            context: "invalid while expression node",
            snippet: node.text().to_string(),
        })?;
    while_expr
        .parts()
        .ok_or_else(|| FormatError::AmbiguousConstruct {
            context: "invalid while expression structure",
            snippet: node.text().to_string(),
        })
}

fn format_while_header(
    parts: &WhileExprParts,
    indent: usize,
    ctx: FormatContext,
) -> Result<String, FormatError> {
    let condition = format_expr_segment(
        &parts.condition_elements,
        "while loop condition",
        indent + 1,
        ctx,
    )?;

    if condition.contains('\n') {
        return Ok(format!(
            "while (\n{}{}\n{})",
            ctx.indent_text(indent + 1),
            condition,
            ctx.indent_text(indent)
        ));
    }

    let inline = format!("while ({condition})");
    if ctx.fits_inline(indent, &inline) {
        return Ok(inline);
    }

    Ok(format!(
        "while (\n{}{}\n{})",
        ctx.indent_text(indent + 1),
        condition,
        ctx.indent_text(indent)
    ))
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

fn format_while_body(
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

fn format_repeat_body(
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

struct RepeatExprParts {
    post_keyword_comments: Vec<String>,
    body: Option<SyntaxElement<RLanguage>>,
}

fn parse_repeat_expr_parts(node: &SyntaxNode) -> Result<RepeatExprParts, FormatError> {
    let elements: Vec<_> = node.children_with_tokens().collect();
    let repeat_idx = elements
        .iter()
        .position(|el| matches!(el, NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::REPEAT_KW))
        .ok_or_else(|| FormatError::AmbiguousConstruct {
            context: "missing 'repeat' keyword",
            snippet: node.text().to_string(),
        })?;

    let mut post_keyword_comments = Vec::new();
    let mut body = None;
    for element in elements.iter().skip(repeat_idx + 1) {
        if is_trivia(element.kind()) {
            continue;
        }
        if let NodeOrToken::Token(tok) = element
            && tok.kind() == SyntaxKind::COMMENT
        {
            post_keyword_comments.push(tok.text().to_string());
            continue;
        }
        body = Some(element.clone());
        break;
    }

    Ok(RepeatExprParts {
        post_keyword_comments,
        body,
    })
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

fn is_comment_only_line(line: &[SyntaxElement<RLanguage>]) -> bool {
    let significant: Vec<_> = line.iter().filter(|el| !is_trivia(el.kind())).collect();
    matches!(
        significant.as_slice(),
        [NodeOrToken::Token(tok)] if tok.kind() == SyntaxKind::COMMENT
    )
}

fn line_starts_with_control_flow_loop(line: &[SyntaxElement<RLanguage>]) -> bool {
    let significant = significant_elements(line);
    match significant.as_slice() {
        [NodeOrToken::Node(node)] => {
            node.kind() == SyntaxKind::FOR_EXPR
                || node.kind() == SyntaxKind::WHILE_EXPR
                || node.kind() == SyntaxKind::REPEAT_EXPR
        }
        [NodeOrToken::Node(node), NodeOrToken::Token(tok)] => {
            (node.kind() == SyntaxKind::FOR_EXPR
                || node.kind() == SyntaxKind::WHILE_EXPR
                || node.kind() == SyntaxKind::REPEAT_EXPR)
                && tok.kind() == SyntaxKind::COMMENT
        }
        _ => false,
    }
}

fn loop_leading_comment_count(
    line: &[SyntaxElement<RLanguage>],
    indent: usize,
    ctx: FormatContext,
) -> Result<Option<usize>, FormatError> {
    let significant = significant_elements(line);
    let (node, kind) = match significant.as_slice() {
        [NodeOrToken::Node(node)] if node.kind() == SyntaxKind::FOR_EXPR => {
            (Some(node.clone()), SyntaxKind::FOR_EXPR)
        }
        [NodeOrToken::Node(node)] if node.kind() == SyntaxKind::WHILE_EXPR => {
            (Some(node.clone()), SyntaxKind::WHILE_EXPR)
        }
        [NodeOrToken::Node(node)] if node.kind() == SyntaxKind::REPEAT_EXPR => {
            (Some(node.clone()), SyntaxKind::REPEAT_EXPR)
        }
        [NodeOrToken::Node(node), NodeOrToken::Token(tok)]
            if node.kind() == SyntaxKind::FOR_EXPR && tok.kind() == SyntaxKind::COMMENT =>
        {
            (Some(node.clone()), SyntaxKind::FOR_EXPR)
        }
        [NodeOrToken::Node(node), NodeOrToken::Token(tok)]
            if node.kind() == SyntaxKind::WHILE_EXPR && tok.kind() == SyntaxKind::COMMENT =>
        {
            (Some(node.clone()), SyntaxKind::WHILE_EXPR)
        }
        [NodeOrToken::Node(node), NodeOrToken::Token(tok)]
            if node.kind() == SyntaxKind::REPEAT_EXPR && tok.kind() == SyntaxKind::COMMENT =>
        {
            (Some(node.clone()), SyntaxKind::REPEAT_EXPR)
        }
        _ => (None, SyntaxKind::ERROR),
    };
    let Some(node) = node else {
        return Ok(None);
    };

    if kind == SyntaxKind::FOR_EXPR {
        return Ok(Some(
            parse_for_expr_parts(&node, indent, ctx)?
                .leading_comments
                .len(),
        ));
    }

    if kind == SyntaxKind::WHILE_EXPR {
        return Ok(Some(
            parse_while_expr_parts(&node, indent, ctx)?
                .leading_comments
                .len(),
        ));
    }

    Ok(Some(
        parse_repeat_expr_parts(&node)?.post_keyword_comments.len(),
    ))
}

use rowan::{NodeOrToken, SyntaxElement};

use super::super::context::FormatContext;
use super::super::core::{
    FormatError, format_block_expr_with_prefixed_comments, format_expr_element,
    format_expr_segment, format_expr_with_optional_comment, is_trivia,
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
    let (mut then_expr, _then_is_block, interstitial_comments, interstitial_attach_to_then) =
        format_if_then_branch_with_comments(&then_elements, indent, ctx)?;
    let then_is_block = branch_starts_with_block(&then_elements);

    let mut out = format!("if ({condition}) {then_expr}");
    if if_expr.else_keyword().is_some() {
        let else_elements =
            if_expr
                .else_elements()
                .ok_or_else(|| FormatError::AmbiguousConstruct {
                    context: "missing else branch",
                    snippet: node.text().to_string(),
                })?;
        let mut else_expr = format_if_branch(&else_elements, indent, ctx, true)?;
        let else_is_block = branch_starts_with_block(&else_elements);
        if !interstitial_comments.is_empty() {
            if then_is_block && interstitial_attach_to_then {
                then_expr =
                    prepend_comments_to_branch(&then_expr, &interstitial_comments, indent, ctx);
                out = format!("if ({condition}) {then_expr}");
            } else {
                else_expr =
                    prepend_comments_to_branch(&else_expr, &interstitial_comments, indent, ctx);
            }
        }
        if then_is_block && !else_is_block {
            else_expr = wrap_branch_in_block(&else_expr, &[], indent, ctx);
        } else if !then_is_block && else_is_block {
            then_expr = wrap_branch_in_block(&then_expr, &[], indent, ctx);
            out = format!("if ({condition}) {then_expr}");
        }
        out.push_str(" else ");
        out.push_str(&else_expr);
    }
    Ok(out)
}

fn format_if_then_branch_with_comments(
    elements: &[SyntaxElement<RLanguage>],
    indent: usize,
    ctx: FormatContext,
) -> Result<(String, bool, Vec<String>, bool), FormatError> {
    let significant = significant_elements(elements);
    if significant.is_empty() {
        return Err(FormatError::AmbiguousConstruct {
            context: "if then branch is empty",
            snippet: String::new(),
        });
    }
    if let Some(first) = significant.first()
        && matches!(first, NodeOrToken::Node(node) if node.kind() == SyntaxKind::BLOCK_EXPR)
    {
        let block =
            format_expr_segment(std::slice::from_ref(first), "if then branch", indent, ctx)?;
        let comments = significant
            .iter()
            .skip(1)
            .filter_map(|el| match el {
                NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::COMMENT => {
                    Some(tok.text().to_string())
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let attach_to_then = comments_attach_to_then_block(elements);
        return Ok((block, true, comments, attach_to_then));
    }
    Ok((
        format_if_branch(elements, indent, ctx, false)?,
        false,
        Vec::new(),
        false,
    ))
}

fn comments_attach_to_then_block(elements: &[SyntaxElement<RLanguage>]) -> bool {
    let mut first_block_idx = None;
    let mut first_comment_idx = None;
    for (idx, el) in elements.iter().enumerate() {
        match el {
            NodeOrToken::Node(node)
                if first_block_idx.is_none() && node.kind() == SyntaxKind::BLOCK_EXPR =>
            {
                first_block_idx = Some(idx);
            }
            NodeOrToken::Token(tok)
                if first_block_idx.is_some()
                    && first_comment_idx.is_none()
                    && tok.kind() == SyntaxKind::COMMENT =>
            {
                first_comment_idx = Some(idx);
                break;
            }
            _ => {}
        }
    }
    let (Some(block_idx), Some(comment_idx)) = (first_block_idx, first_comment_idx) else {
        return false;
    };
    !elements[block_idx + 1..comment_idx]
        .iter()
        .any(|el| el.kind() == SyntaxKind::NEWLINE)
}

fn branch_starts_with_block(elements: &[SyntaxElement<RLanguage>]) -> bool {
    significant_elements(elements).first().is_some_and(
        |el| matches!(el, NodeOrToken::Node(node) if node.kind() == SyntaxKind::BLOCK_EXPR),
    )
}

fn prepend_comments_to_branch(
    rendered: &str,
    comments: &[String],
    indent: usize,
    ctx: FormatContext,
) -> String {
    if comments.is_empty() {
        return rendered.to_string();
    }
    if let Some(inner) = rendered
        .strip_prefix("{\n")
        .and_then(|rest| rest.strip_suffix("\n}"))
    {
        let mut out = String::from("{\n");
        for comment in comments {
            out.push_str(&ctx.indent_text(indent + 1));
            out.push_str(comment);
            out.push('\n');
        }
        if !inner.is_empty() {
            out.push_str(inner);
            out.push('\n');
        }
        out.push_str(&ctx.indent_text(indent));
        out.push('}');
        return out;
    }
    wrap_branch_in_block(rendered, comments, indent, ctx)
}

fn wrap_branch_in_block(
    rendered: &str,
    comments: &[String],
    indent: usize,
    ctx: FormatContext,
) -> String {
    if rendered == "{}" && comments.is_empty() {
        return "{}".to_string();
    }
    let mut out = String::from("{\n");
    for comment in comments {
        out.push_str(&ctx.indent_text(indent + 1));
        out.push_str(comment);
        out.push('\n');
    }
    if rendered != "{}" {
        for line in rendered.lines() {
            out.push_str(&ctx.indent_text(indent + 1));
            out.push_str(line);
            out.push('\n');
        }
    }
    out.push_str(&ctx.indent_text(indent));
    out.push('}');
    out
}

fn format_if_branch(
    elements: &[SyntaxElement<RLanguage>],
    indent: usize,
    ctx: FormatContext,
    keep_trailing_comment: bool,
) -> Result<String, FormatError> {
    let significant: Vec<_> = elements
        .iter()
        .filter(|el| !is_trivia(el.kind()))
        .cloned()
        .collect();
    if significant.is_empty() {
        return Err(FormatError::AmbiguousConstruct {
            context: "if branch is empty",
            snippet: String::new(),
        });
    }

    let mut start = 0usize;
    let mut leading_comments = Vec::new();
    while start < significant.len() {
        match &significant[start] {
            NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::COMMENT => {
                leading_comments.push(tok.text().to_string());
                start += 1;
            }
            _ => break,
        }
    }

    let mut end = significant.len();
    let trailing_comment = if keep_trailing_comment
        && end > start
        && matches!(
            significant.last(),
            Some(NodeOrToken::Token(tok)) if tok.kind() == SyntaxKind::COMMENT
        ) {
        end -= 1;
        match &significant[end] {
            NodeOrToken::Token(tok) => Some(tok.text().to_string()),
            NodeOrToken::Node(_) => None,
        }
    } else {
        None
    };

    let core = &significant[start..end];
    if core.is_empty() {
        return Ok(match trailing_comment {
            Some(comment) => comment,
            None => "{}".to_string(),
        });
    }

    let mut rendered = if core.len() == 1 {
        match &core[0] {
            NodeOrToken::Node(node) if node.kind() == SyntaxKind::BLOCK_EXPR => {
                format_block_expr_with_prefixed_comments(node, indent, ctx, &leading_comments)?
            }
            _ => {
                if leading_comments.is_empty() {
                    format_expr_with_optional_comment(core, "if branch", indent, ctx)?
                } else {
                    let expr =
                        format_expr_with_optional_comment(core, "if branch", indent + 1, ctx)?;
                    let mut out = String::from("{\n");
                    for comment in &leading_comments {
                        out.push_str(&ctx.indent_text(indent + 1));
                        out.push_str(comment);
                        out.push('\n');
                    }
                    out.push_str(&ctx.indent_text(indent + 1));
                    out.push_str(&expr);
                    out.push('\n');
                    out.push_str(&ctx.indent_text(indent));
                    out.push('}');
                    out
                }
            }
        }
    } else if leading_comments.is_empty() {
        format_expr_with_optional_comment(core, "if branch", indent, ctx)?
    } else {
        let expr = format_expr_with_optional_comment(core, "if branch", indent + 1, ctx)?;
        let mut out = String::from("{\n");
        for comment in &leading_comments {
            out.push_str(&ctx.indent_text(indent + 1));
            out.push_str(comment);
            out.push('\n');
        }
        out.push_str(&ctx.indent_text(indent + 1));
        out.push_str(&expr);
        out.push('\n');
        out.push_str(&ctx.indent_text(indent));
        out.push('}');
        out
    };

    if let Some(comment) = trailing_comment {
        rendered.push(' ');
        rendered.push_str(&comment);
    }
    Ok(rendered)
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

pub(crate) fn try_format_if_with_external_body(
    lines: &[Vec<SyntaxElement<RLanguage>>],
    line_idx: usize,
    indent: usize,
    ctx: FormatContext,
) -> Result<Option<(String, usize)>, FormatError> {
    let significant = significant_elements(&lines[line_idx]);
    let (if_node, trailing_comment) = match significant.as_slice() {
        [NodeOrToken::Node(node)] if node.kind() == SyntaxKind::IF_EXPR => (node.clone(), None),
        [NodeOrToken::Node(node), NodeOrToken::Token(tok)]
            if node.kind() == SyntaxKind::IF_EXPR && tok.kind() == SyntaxKind::COMMENT =>
        {
            (node.clone(), Some(tok.text().to_string()))
        }
        _ => return Ok(None),
    };
    let if_expr = IfExpr::cast(if_node).ok_or_else(|| FormatError::AmbiguousConstruct {
        context: "invalid if expression node",
        snippet: String::new(),
    })?;
    if if_expr.else_keyword().is_some() {
        return Ok(None);
    }

    let then_elements = if_expr
        .then_elements()
        .ok_or_else(|| FormatError::AmbiguousConstruct {
            context: "missing if branch",
            snippet: String::new(),
        })?;
    let then_significant = significant_elements(&then_elements);
    let then_comment = match then_significant.as_slice() {
        [NodeOrToken::Token(tok)] if tok.kind() == SyntaxKind::COMMENT => tok.text().to_string(),
        _ => return Ok(None),
    };

    let mut cursor = line_idx + 1;
    while cursor < lines.len() && significant_elements(&lines[cursor]).is_empty() {
        cursor += 1;
    }
    if cursor >= lines.len() {
        return Ok(None);
    }
    let body_element = match significant_elements(&lines[cursor]).as_slice() {
        [el] => el.clone(),
        _ => return Ok(None),
    };

    let condition_elements =
        if_expr
            .condition_elements()
            .ok_or_else(|| FormatError::AmbiguousConstruct {
                context: "missing if condition",
                snippet: String::new(),
            })?;
    let condition = format_expr_segment(&condition_elements, "if condition", indent, ctx)?;
    let body_expr = format_expr_element(&body_element, indent + 1, ctx)?;
    let body = wrap_branch_in_block(&body_expr, &[then_comment], indent, ctx);
    let mut out = format!("if ({condition}) {body}");
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

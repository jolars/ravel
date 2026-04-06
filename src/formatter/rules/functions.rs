use rowan::{NodeOrToken, SyntaxElement};

use super::super::context::FormatContext;
use super::super::core::{
    FormatError, format_expr_element, format_expr_segment, format_expr_with_optional_comment,
    snippet_from_elements,
};
use crate::parser::parse;
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

    if !parts.has_non_empty_arg && parts.comma_count == 0 {
        return Ok(format!("{callee}()"));
    }

    if let Some(hugged) = try_hug_single_argument_call(&callee, &parts, indent, ctx) {
        return Ok(hugged);
    }

    if let Some(inline) = try_format_call_with_trailing_function(&callee, &parts, indent, ctx)?
        && !parts.has_comment_arg
        && ctx.fits_with_newlines(indent, &inline)
    {
        return Ok(inline);
    }

    if let Some(inline) = try_format_call_with_trailing_block(&callee, &parts, indent, ctx)?
        && !parts.has_comment_arg
        && ctx.fits_with_newlines(indent, &inline)
    {
        return Ok(inline);
    }

    let formatted_args = format_arg_list_from_parts(&parts, &arg_list)?;
    let inline = format!("{callee}({formatted_args})");
    if !parts.has_comment_arg
        && !should_force_multiline_for_named_function_args(&parts)
        && ctx.fits_inline(indent, &inline)
    {
        return Ok(inline);
    }

    let multiline_args = format_arg_list_multiline(&arg_list, indent, ctx)?;
    Ok(format!(
        "{callee}(\n{multiline_args}\n{})",
        ctx.indent_text(indent)
    ))
}

fn try_hug_single_argument_call(
    callee: &str,
    parts: &CallArgParts,
    indent: usize,
    ctx: FormatContext,
) -> Option<String> {
    if parts.comma_count != 0 || parts.arg_infos.len() != 1 {
        return None;
    }
    let arg = &parts.arg_infos[0];
    if arg.is_comment_only || arg.formatted.is_empty() {
        return None;
    }
    if arg.is_named {
        return None;
    }
    if !arg.formatted.contains('\n') {
        return None;
    }

    let normalized = normalize_empty_call_newlines(&arg.formatted);
    if normalized.trim_start().starts_with('{') {
        return None;
    }
    if !normalized.contains('\n') {
        let candidate = format!("{callee}({normalized})");
        if ctx.fits_with_newlines(indent, &candidate) {
            return Some(candidate);
        }
    }

    Some(format!("{callee}({normalized})"))
}

fn normalize_empty_call_newlines(formatted: &str) -> String {
    let lines: Vec<&str> = formatted.lines().collect();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0usize;
    while i < lines.len() {
        if i + 2 < lines.len()
            && lines[i].trim_end().ends_with('(')
            && lines[i + 1].trim().is_empty()
            && lines[i + 2].trim() == ")"
        {
            out.push(format!("{}{})", lines[i], ""));
            i += 3;
            continue;
        }
        out.push(lines[i].to_string());
        i += 1;
    }
    out.join("\n")
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
                if arg.trim_end().ends_with('=') {
                    out.push_str(" ,");
                } else {
                    out.push(',');
                }
            } else if arg.trim_end().ends_with('=') {
                out.push_str(" , ");
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
                if let Some(CallItem::Arg(next_arg)) = items.get(i + 1)
                    && !arg.is_comment_only
                    && is_assignment_continuation(&next_arg.formatted)
                {
                    for line in merge_named_arg_continuation(
                        &arg.formatted,
                        &next_arg.formatted,
                        &item_indent,
                    ) {
                        out.push(line);
                    }
                    i += 2;
                    continue;
                }

                if let Some(CallItem::Arg(comment_arg)) = items.get(i + 1)
                    && comment_arg.is_comment_only
                    && !comment_arg.leading_newline
                {
                    out.push(append_trailing_comment(
                        &arg.formatted,
                        &comment_arg.formatted,
                        &item_indent,
                        false,
                    ));
                    i += 2;
                    continue;
                }

                if let (
                    Some(CallItem::Comma {
                        newline_after: false,
                    }),
                    Some(CallItem::Arg(comment_arg)),
                ) = (items.get(i + 1), items.get(i + 2))
                    && comment_arg.is_comment_only
                {
                    let sep = if arg.formatted.trim_end().ends_with('=') {
                        " , "
                    } else {
                        ", "
                    };
                    out.push(format!(
                        "{item_indent}{}{sep}{}",
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
                    out.push(indent_multiline_arg(&arg.formatted, &item_indent, true));
                    i += 2;
                } else {
                    out.push(indent_multiline_arg(&arg.formatted, &item_indent, false));
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

fn indent_multiline_arg(formatted: &str, item_indent: &str, trailing_comma: bool) -> String {
    let mut lines: Vec<String> = formatted.lines().map(ToString::to_string).collect();
    if lines.is_empty() {
        return format!("{item_indent}{}", if trailing_comma { "," } else { "" });
    }

    let item_indent_len = item_indent.len();
    if lines.len() > 1
        && let Some(expr_start) = lines
            .iter()
            .position(|line| !line.trim().is_empty() && !line.trim_start().starts_with('#'))
    {
        let min_rest_indent = lines
            .iter()
            .skip(expr_start + 1)
            .filter(|line| !line.trim().is_empty())
            .map(|line| line.chars().take_while(|c| *c == ' ').count())
            .min()
            .unwrap_or(0);
        if min_rest_indent >= item_indent_len {
            for line in lines.iter_mut().skip(expr_start + 1) {
                if line.len() >= item_indent_len {
                    *line = line[item_indent_len..].to_string();
                }
            }
        }
    }

    for line in &mut lines {
        *line = format!("{item_indent}{line}");
    }
    if trailing_comma && let Some(last) = lines.last_mut() {
        append_argument_comma(last);
    }
    lines.join("\n")
}

fn append_argument_comma(text: &mut String) {
    if text.trim_end().ends_with('=') {
        text.push_str(" ,");
    } else {
        text.push(',');
    }
}

fn is_assignment_continuation(formatted: &str) -> bool {
    let mut saw_comment = false;
    for line in formatted.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with('#') {
            saw_comment = true;
            continue;
        }
        if trimmed.starts_with('=') {
            return true;
        }
        if saw_comment {
            return false;
        }
        return false;
    }
    false
}

fn merge_named_arg_continuation(name: &str, continuation: &str, item_indent: &str) -> Vec<String> {
    let lines: Vec<&str> = continuation.lines().collect();
    let mut out = Vec::new();
    if lines.is_empty() {
        out.push(format!("{item_indent}{name}"));
        return out;
    }

    let mut name_lines = name.lines().map(str::trim).filter(|l| !l.is_empty());
    let mut comments = Vec::new();
    let mut lhs = None::<String>;
    for line in name_lines.by_ref() {
        if line.starts_with('#') {
            comments.push(line.to_string());
        } else if lhs.is_none() {
            lhs = Some(line.to_string());
        }
    }
    let mut rhs_idx = 0usize;
    let mut rhs_comments = Vec::new();
    while rhs_idx < lines.len() {
        let trimmed = lines[rhs_idx].trim();
        if trimmed.is_empty() {
            rhs_idx += 1;
            continue;
        }
        if trimmed.starts_with('#') {
            rhs_comments.push(trimmed.to_string());
            rhs_idx += 1;
            continue;
        }
        break;
    }

    for comment in comments {
        out.push(format!("{item_indent}{comment}"));
    }
    for comment in rhs_comments {
        out.push(format!("{item_indent}{comment}"));
    }

    if rhs_idx >= lines.len() {
        out.push(format!(
            "{item_indent}{}",
            lhs.unwrap_or_else(|| name.trim().to_string())
        ));
        return out;
    }

    let first_rhs = lines[rhs_idx].trim_start();
    let lhs_text = lhs.unwrap_or_else(|| name.trim().to_string());
    out.push(format!("{item_indent}{lhs_text} {first_rhs}"));
    let tail = &lines[rhs_idx + 1..];
    let min_tail_indent = tail
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.chars().take_while(|c| *c == ' ').count())
        .min()
        .unwrap_or(0);
    for line in tail {
        let normalized = if line.len() >= min_tail_indent {
            &line[min_tail_indent..]
        } else {
            line
        };
        out.push(format!("{item_indent}{normalized}"));
    }
    out
}

fn append_trailing_comment(
    formatted: &str,
    comment: &str,
    item_indent: &str,
    trailing_comma: bool,
) -> String {
    let mut lines: Vec<String> = formatted.lines().map(ToString::to_string).collect();
    if lines.is_empty() {
        return format!("{item_indent}{comment}");
    }
    if let Some(first) = lines.first_mut() {
        *first = format!("{item_indent}{first}");
    }
    if let Some(last) = lines.last_mut() {
        last.push(' ');
        last.push_str(comment);
        if trailing_comma {
            last.push(',');
        }
    }
    lines.join("\n")
}

enum CallItem {
    Arg(ArgInfo),
    Comma { newline_after: bool },
}

struct ArgInfo {
    formatted: String,
    is_empty: bool,
    is_comment_only: bool,
    leading_newline: bool,
    is_named: bool,
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
                let is_named = is_named_arg_node(arg);
                let leading_newline = has_newline_before_arg(&elements, idx);
                items.push(CallItem::Arg(ArgInfo {
                    formatted,
                    is_empty,
                    is_comment_only,
                    leading_newline,
                    is_named,
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
    arg_infos: Vec<ArgInfo>,
    comma_count: usize,
    has_non_empty_arg: bool,
    has_comment_arg: bool,
}

fn should_force_multiline_for_named_function_args(parts: &CallArgParts) -> bool {
    let non_empty_count = parts
        .arg_infos
        .iter()
        .filter(|arg| !arg.formatted.is_empty())
        .count();
    if non_empty_count <= 1 {
        return false;
    }
    let named_function_args = parts
        .arg_infos
        .iter()
        .filter(|arg| arg.is_named && arg.formatted.contains("function("))
        .count();
    named_function_args >= 2
}

fn collect_call_arg_parts(
    node: &SyntaxNode,
    indent: usize,
    ctx: FormatContext,
) -> Result<CallArgParts, FormatError> {
    let mut formatted_args = Vec::new();
    let mut arg_infos = Vec::new();
    let mut comment_arg_mask = Vec::new();
    let mut comma_count = 0usize;

    for element in node.children_with_tokens() {
        match element {
            NodeOrToken::Node(arg) if arg.kind() == SyntaxKind::ARG => {
                let formatted = format_arg(&arg, indent, ctx)?;
                let is_comment_only = is_comment_only_arg(&arg);
                comment_arg_mask.push(is_comment_only);
                formatted_args.push(formatted.clone());
                arg_infos.push(ArgInfo {
                    formatted,
                    is_empty: false,
                    is_comment_only,
                    leading_newline: false,
                    is_named: is_named_arg_node(&arg),
                });
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
        arg_infos,
        comma_count,
        has_non_empty_arg,
        has_comment_arg,
    })
}

fn has_newline_before_arg(elements: &[SyntaxElement<RLanguage>], idx: usize) -> bool {
    for prev in elements[..idx].iter().rev() {
        match prev {
            NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::NEWLINE => return true,
            NodeOrToken::Token(tok)
                if tok.kind() == SyntaxKind::WHITESPACE || tok.kind() == SyntaxKind::COMMENT => {}
            NodeOrToken::Token(tok)
                if tok.kind() == SyntaxKind::COMMA || tok.kind() == SyntaxKind::LPAREN =>
            {
                return false;
            }
            NodeOrToken::Node(n) if n.kind() == SyntaxKind::ARG => return false,
            _ => return false,
        }
    }
    false
}

fn is_named_arg_node(node: &SyntaxNode) -> bool {
    node.children_with_tokens().any(|el| {
        matches!(
            el,
            NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::ASSIGN_EQ
        )
    })
}

fn try_format_call_with_trailing_block(
    callee: &str,
    parts: &CallArgParts,
    indent: usize,
    ctx: FormatContext,
) -> Result<Option<String>, FormatError> {
    if parts.arg_infos.is_empty() || parts.arg_infos.len() != parts.comma_count + 1 {
        return Ok(None);
    }
    if parts.arg_infos.iter().any(|arg| arg.is_comment_only) {
        return Ok(None);
    }
    if parts.arg_infos.iter().any(|arg| arg.formatted.is_empty()) {
        return Ok(None);
    }

    let Some((last, leading)) = parts.arg_infos.split_last() else {
        return Ok(None);
    };
    if !looks_like_trailing_block_arg(&last.formatted) {
        return Ok(None);
    }
    if last
        .formatted
        .lines()
        .find(|line| !line.trim().is_empty())
        .map(|line| line.trim_start().starts_with('#'))
        .unwrap_or(false)
    {
        return Ok(None);
    }
    if leading.iter().any(|arg| arg.formatted.contains('\n')) {
        return Ok(None);
    }
    let inline_leading = leading
        .iter()
        .map(|arg| arg.formatted.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let candidate = if inline_leading.is_empty() {
        format!("{callee}({})", last.formatted)
    } else {
        format!("{callee}({inline_leading}, {})", last.formatted)
    };
    if ctx.fits_with_newlines(indent, &candidate) {
        return Ok(Some(candidate));
    }
    Ok(None)
}

fn try_format_call_with_trailing_function(
    callee: &str,
    parts: &CallArgParts,
    indent: usize,
    ctx: FormatContext,
) -> Result<Option<String>, FormatError> {
    if parts.arg_infos.is_empty() || parts.arg_infos.len() != parts.comma_count + 1 {
        return Ok(None);
    }
    if parts.arg_infos.iter().any(|arg| arg.is_comment_only) {
        return Ok(None);
    }
    if parts.arg_infos.iter().any(|arg| arg.formatted.is_empty()) {
        return Ok(None);
    }

    let Some((last, leading)) = parts.arg_infos.split_last() else {
        return Ok(None);
    };
    if !(last.formatted.starts_with("function(") || last.formatted.starts_with("\\(")) {
        return Ok(None);
    }
    if leading.iter().any(|arg| arg.formatted.contains('\n')) {
        return Ok(None);
    }

    let inline_leading = leading
        .iter()
        .map(|arg| arg.formatted.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let candidate = if inline_leading.is_empty() {
        format!("{callee}({})", last.formatted)
    } else {
        format!("{callee}({inline_leading}, {})", last.formatted)
    };
    if ctx.fits_with_newlines(indent, &candidate) {
        return Ok(Some(candidate));
    }
    Ok(None)
}

fn looks_like_trailing_block_arg(text: &str) -> bool {
    (text.starts_with('{') || text.contains(" = {")) && text.ends_with('}')
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

    if let [NodeOrToken::Node(assign)] = significant.as_slice()
        && assign.kind() == SyntaxKind::ASSIGNMENT_EXPR
    {
        return format_assignment_expr_arg(assign, indent, ctx);
    }

    if let Some(curly_curly) = try_format_curly_curly(&significant, indent, ctx)? {
        return Ok(curly_curly);
    }

    if let Some(eq_idx) = elements
        .iter()
        .position(|el| matches!(el, NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::ASSIGN_EQ))
    {
        let lhs_comments: Vec<String> = elements[..eq_idx]
            .iter()
            .filter_map(|el| match el {
                NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::COMMENT => {
                    Some(tok.text().to_string())
                }
                _ => None,
            })
            .collect();
        let lhs_significant: Vec<_> = elements[..eq_idx]
            .iter()
            .filter(|el| {
                !super::super::core::is_trivia(el.kind()) && el.kind() != SyntaxKind::COMMENT
            })
            .cloned()
            .collect();
        let name = if lhs_significant.is_empty() {
            String::new()
        } else {
            format_expr_segment(&lhs_significant, "named arg name", indent, ctx)?
        };
        let (leading_comments, value) =
            format_assignment_rhs_with_leading_comments(&elements[eq_idx + 1..], indent, ctx)?;
        let base = format_named_assignment(&name, &value);
        let mut all_comments = lhs_comments;
        all_comments.extend(leading_comments);
        if all_comments.is_empty() {
            return Ok(base);
        }
        return Ok(format!("{}\n{base}", all_comments.join("\n")));
    }

    let significant: Vec<_> = elements
        .iter()
        .filter(|el| !super::super::core::is_trivia(el.kind()))
        .cloned()
        .collect();
    if let [NodeOrToken::Token(name_tok), NodeOrToken::Node(assign_node)] = significant.as_slice()
        && name_tok.kind() == SyntaxKind::IDENT
        && assign_node.kind() == SyntaxKind::ASSIGNMENT_EXPR
    {
        return format_named_arg_with_assignment_node(name_tok.text(), assign_node, indent, ctx);
    }

    format_expr_with_optional_comment(&elements, "positional arg", indent, ctx)
}

fn try_format_curly_curly(
    significant: &[SyntaxElement<RLanguage>],
    indent: usize,
    ctx: FormatContext,
) -> Result<Option<String>, FormatError> {
    let [NodeOrToken::Node(outer)] = significant else {
        return Ok(None);
    };
    if outer.kind() != SyntaxKind::BLOCK_EXPR {
        return Ok(None);
    }

    let outer_significant: Vec<_> = outer
        .children_with_tokens()
        .filter(|el| !super::super::core::is_trivia(el.kind()))
        .collect();
    if outer_significant.len() != 3 {
        return Ok(None);
    }
    let [
        NodeOrToken::Token(outer_l),
        NodeOrToken::Node(inner),
        NodeOrToken::Token(outer_r),
    ] = outer_significant.as_slice()
    else {
        return Ok(None);
    };
    if outer_l.kind() != SyntaxKind::LBRACE || outer_r.kind() != SyntaxKind::RBRACE {
        return Ok(None);
    }
    if inner.kind() != SyntaxKind::BLOCK_EXPR {
        return Ok(None);
    }

    let inner_significant: Vec<_> = inner
        .children_with_tokens()
        .filter(|el| !super::super::core::is_trivia(el.kind()))
        .collect();
    if inner_significant.len() < 2 {
        return Ok(None);
    }
    let Some(NodeOrToken::Token(inner_l)) = inner_significant.first() else {
        return Ok(None);
    };
    let Some(NodeOrToken::Token(inner_r)) = inner_significant.last() else {
        return Ok(None);
    };
    if inner_l.kind() != SyntaxKind::LBRACE || inner_r.kind() != SyntaxKind::RBRACE {
        return Ok(None);
    }

    let inner_body = &inner_significant[1..inner_significant.len() - 1];
    if inner_body.is_empty() {
        return Ok(None);
    }
    let body = format_expr_segment(inner_body, "curly-curly inner body", indent, ctx)?;
    if body.contains('\n') || body.trim_start().starts_with('#') {
        return Ok(None);
    }
    Ok(Some(format!("{{{{ {body} }}}}")))
}

fn format_named_arg_with_assignment_node(
    name: &str,
    assign_node: &SyntaxNode,
    indent: usize,
    ctx: FormatContext,
) -> Result<String, FormatError> {
    let elements: Vec<_> = assign_node.children_with_tokens().collect();
    let eq_idx = elements
        .iter()
        .position(|el| matches!(el, NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::ASSIGN_EQ))
        .ok_or_else(|| FormatError::AmbiguousConstruct {
            context: "missing '=' in assignment node for named arg",
            snippet: assign_node.text().to_string(),
        })?;

    let leading_comments: Vec<String> = elements[..eq_idx]
        .iter()
        .filter_map(|el| match el {
            NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::COMMENT => {
                Some(tok.text().to_string())
            }
            _ => None,
        })
        .collect();
    let (rhs_leading_comments, value) =
        format_assignment_rhs_with_leading_comments(&elements[eq_idx + 1..], indent, ctx)?;
    let base = format_named_assignment(name, &value);
    let mut all_comments = leading_comments;
    all_comments.extend(rhs_leading_comments);
    if all_comments.is_empty() {
        return Ok(base);
    }
    Ok(format!("{}\n{base}", all_comments.join("\n")))
}

fn format_assignment_expr_arg(
    node: &SyntaxNode,
    indent: usize,
    ctx: FormatContext,
) -> Result<String, FormatError> {
    let elements: Vec<_> = node.children_with_tokens().collect();
    let eq_idx = elements
        .iter()
        .position(|el| matches!(el, NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::ASSIGN_EQ))
        .ok_or_else(|| FormatError::AmbiguousConstruct {
            context: "missing '=' in assignment expr arg",
            snippet: node.text().to_string(),
        })?;

    let leading_comments: Vec<String> = elements[..eq_idx]
        .iter()
        .filter_map(|el| match el {
            NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::COMMENT => {
                Some(tok.text().to_string())
            }
            _ => None,
        })
        .collect();
    let lhs_significant: Vec<_> = elements[..eq_idx]
        .iter()
        .filter(|el| !super::super::core::is_trivia(el.kind()) && el.kind() != SyntaxKind::COMMENT)
        .cloned()
        .collect();
    let (rhs_leading_comments, rhs) =
        format_assignment_rhs_with_leading_comments(&elements[eq_idx + 1..], indent, ctx)?;

    let tail = if lhs_significant.is_empty() {
        if rhs.is_empty() {
            "=".to_string()
        } else {
            format!("= {rhs}")
        }
    } else {
        let lhs = format_expr_segment(&elements[..eq_idx], "assignment expr arg lhs", indent, ctx)?;
        format_named_assignment(&lhs, &rhs)
    };
    let mut all_comments = leading_comments;
    all_comments.extend(rhs_leading_comments);
    if all_comments.is_empty() {
        return Ok(tail);
    }
    Ok(format!("{}\n{tail}", all_comments.join("\n")))
}

fn format_assignment_rhs_with_leading_comments(
    elements: &[SyntaxElement<RLanguage>],
    indent: usize,
    ctx: FormatContext,
) -> Result<(Vec<String>, String), FormatError> {
    let mut idx = 0usize;
    let mut leading_comments = Vec::new();
    while idx < elements.len() {
        match &elements[idx] {
            NodeOrToken::Token(tok) if super::super::core::is_trivia(tok.kind()) => {
                idx += 1;
            }
            NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::COMMENT => {
                leading_comments.push(tok.text().to_string());
                idx += 1;
            }
            _ => break,
        }
    }
    if idx >= elements.len() {
        return Ok((leading_comments, String::new()));
    }
    let value = format_expr_with_optional_comment(&elements[idx..], "assignment rhs", indent, ctx)?;
    Ok((leading_comments, value))
}

fn format_named_assignment(name: &str, value: &str) -> String {
    if name.is_empty() {
        return if value.is_empty() {
            "=".to_string()
        } else {
            format!("= {value}")
        };
    }
    if value.is_empty() {
        format!("{name} =")
    } else {
        format!("{name} = {value}")
    }
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
    let function_head = match &elements[fn_idx] {
        NodeOrToken::Token(tok) if tok.text() == "\\" => "\\",
        _ => "function",
    };
    let leading_fn_comments = elements[fn_idx + 1..]
        .iter()
        .take_while(|el| !matches!(el, NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::LPAREN))
        .filter_map(|el| match el {
            NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::COMMENT => {
                Some(tok.text().to_string())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
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
    let body_elements = &elements[rparen_idx + 1..];
    let mut body_leading_comments = Vec::new();
    let mut body_start_idx = 0usize;
    while body_start_idx < body_elements.len() {
        match &body_elements[body_start_idx] {
            NodeOrToken::Token(tok) if super::super::core::is_trivia(tok.kind()) => {
                body_start_idx += 1;
            }
            NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::COMMENT => {
                body_leading_comments.push(tok.text().to_string());
                body_start_idx += 1;
            }
            _ => break,
        }
    }
    let body_core = &body_elements[body_start_idx..];
    if body_core.is_empty() {
        return Err(FormatError::AmbiguousConstruct {
            context: "missing function body expression",
            snippet: node.text().to_string(),
        });
    }

    let body = format_expr_segment(body_core, "function body", indent, ctx)?;
    let body_significant: Vec<_> = body_core
        .iter()
        .filter(|el| !super::super::core::is_trivia(el.kind()) && el.kind() != SyntaxKind::COMMENT)
        .cloned()
        .collect();
    let body_is_block = matches!(
        body_significant.as_slice(),
        [NodeOrToken::Node(n)] if n.kind() == SyntaxKind::BLOCK_EXPR
    );
    if body_is_block {
        if body_leading_comments.is_empty()
            && !params.contains('\n')
            && let Some(flat_body) = flatten_simple_formatted_block(&body)
        {
            let inline = format!("{function_head}({params}) {flat_body}");
            if ctx.fits_with_newlines(indent, &inline) {
                return Ok(prepend_function_leading_comments(
                    inline,
                    &leading_fn_comments,
                    indent,
                    ctx,
                ));
            }
        }
        let body_block =
            prepend_comments_to_formatted_block(&body, &body_leading_comments, indent, ctx);
        let rendered = format!("{function_head}({params}) {body_block}");
        return Ok(prepend_function_leading_comments(
            rendered,
            &leading_fn_comments,
            indent,
            ctx,
        ));
    }

    let inline = format!("{function_head}({params}) {body}");
    if body_leading_comments.is_empty()
        && !params.contains('\n')
        && ctx.fits_with_newlines(indent, &inline)
    {
        return Ok(prepend_function_leading_comments(
            inline,
            &leading_fn_comments,
            indent,
            ctx,
        ));
    }

    let body_line = format_expr_segment(body_core, "function body", indent + 1, ctx)?;
    let mut block_lines = Vec::new();
    for comment in body_leading_comments {
        block_lines.push(format!("{}{}", ctx.indent_text(indent + 1), comment));
    }
    block_lines.push(format!("{}{}", ctx.indent_text(indent + 1), body_line));
    let rendered = format!(
        "{function_head}({params}) {{\n{}\n{}}}",
        block_lines.join("\n"),
        ctx.indent_text(indent)
    );
    Ok(prepend_function_leading_comments(
        rendered,
        &leading_fn_comments,
        indent,
        ctx,
    ))
}

fn prepend_comments_to_formatted_block(
    block: &str,
    comments: &[String],
    indent: usize,
    ctx: FormatContext,
) -> String {
    if comments.is_empty() {
        return block.to_string();
    }
    let comment_lines = comments
        .iter()
        .map(|comment| format!("{}{}", ctx.indent_text(indent + 1), comment))
        .collect::<Vec<_>>()
        .join("\n");
    if block == "{}" {
        return format!("{{\n{comment_lines}\n{}}}", ctx.indent_text(indent));
    }
    if let Some(inner) = block
        .strip_prefix("{\n")
        .and_then(|rest| rest.strip_suffix("\n}"))
    {
        if inner.is_empty() {
            return format!("{{\n{comment_lines}\n{}}}", ctx.indent_text(indent));
        }
        return format!(
            "{{\n{comment_lines}\n{inner}\n{}}}",
            ctx.indent_text(indent)
        );
    }
    format!(
        "{{\n{comment_lines}\n{}\n{}}}",
        block,
        ctx.indent_text(indent)
    )
}

fn flatten_simple_formatted_block(block: &str) -> Option<String> {
    let inner = block.strip_prefix("{\n")?.strip_suffix("\n}")?;
    let lines = inner
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    if lines.len() != 1 {
        return None;
    }
    let line = lines[0];
    if line.starts_with('#') || line.contains('#') {
        return None;
    }
    Some(line.to_string())
}

fn prepend_function_leading_comments(
    rendered: String,
    comments: &[String],
    indent: usize,
    ctx: FormatContext,
) -> String {
    if comments.is_empty() {
        return rendered;
    }
    let prefix = comments
        .iter()
        .map(|comment| format!("{}{}", ctx.indent_text(indent), comment))
        .collect::<Vec<_>>()
        .join("\n");
    format!("{prefix}\n{rendered}")
}

fn format_function_parameters(
    elements: &[SyntaxElement<RLanguage>],
    indent: usize,
    ctx: FormatContext,
) -> Result<String, FormatError> {
    let has_param_comment = elements
        .iter()
        .any(|el| matches!(el, NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::COMMENT));
    if !has_param_comment {
        return format_function_parameters_without_comments(elements, indent, ctx);
    }

    let param_segments = split_top_level_function_params(elements);
    if param_segments.is_empty() {
        return Ok(String::new());
    }
    let mut multiline = String::new();
    multiline.push('\n');
    for (idx, segment) in param_segments.iter().enumerate() {
        let raw = snippet_from_elements(segment);
        let lines = raw
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>();
        if lines.is_empty() {
            continue;
        }
        for (line_idx, line) in lines.iter().enumerate() {
            multiline.push_str(&ctx.indent_text(indent + 1));
            multiline.push_str(line);
            if idx + 1 < param_segments.len() && line_idx + 1 == lines.len() {
                multiline.push(',');
            }
            multiline.push('\n');
        }
    }
    multiline.push_str(&ctx.indent_text(indent));
    Ok(multiline)
}

fn split_top_level_function_params(
    elements: &[SyntaxElement<RLanguage>],
) -> Vec<Vec<SyntaxElement<RLanguage>>> {
    let mut segments = Vec::new();
    let mut current = Vec::new();
    let mut depth = 0usize;

    for element in elements {
        match element {
            NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::COMMA && depth == 0 => {
                if !current.is_empty() {
                    segments.push(std::mem::take(&mut current));
                }
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
                current.push(element.clone());
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
                current.push(element.clone());
            }
            _ => current.push(element.clone()),
        }
    }
    if !current.is_empty() {
        segments.push(current);
    }
    segments
}

fn format_function_parameters_without_comments(
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
    for param in &params {
        out.push(format_function_parameter(param, indent, ctx)?);
    }
    let inline = out.join(", ");
    if ctx.fits_with_newlines(indent, &format!("function({inline}) {{}}")) {
        return Ok(inline);
    }

    let mut multiline = String::new();
    multiline.push('\n');
    for (idx, param) in out.iter().enumerate() {
        multiline.push_str(&ctx.indent_text(indent + 1));
        multiline.push_str(param);
        if idx + 1 < out.len() {
            multiline.push(',');
        }
        multiline.push('\n');
    }
    multiline.push_str(&ctx.indent_text(indent));
    Ok(multiline)
}

fn format_function_parameter(
    elements: &[SyntaxElement<RLanguage>],
    indent: usize,
    ctx: FormatContext,
) -> Result<String, FormatError> {
    if let Some(eq_idx) = elements
        .iter()
        .position(|el| matches!(el, NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::ASSIGN_EQ))
    {
        let name =
            format_expr_segment(&elements[..eq_idx], "function parameter name", indent, ctx)?;
        let value = format_expr_or_braced_tokens(
            &elements[eq_idx + 1..],
            "function parameter default",
            indent,
            ctx,
        )?;
        return Ok(format!("{name} = {value}"));
    }

    format_expr_segment(elements, "function parameter", indent, ctx)
}

fn format_expr_or_braced_tokens(
    elements: &[SyntaxElement<RLanguage>],
    context: &'static str,
    indent: usize,
    ctx: FormatContext,
) -> Result<String, FormatError> {
    let significant: Vec<_> = elements
        .iter()
        .filter(|el| !super::super::core::is_trivia(el.kind()))
        .cloned()
        .collect();
    if significant.is_empty() {
        return Err(FormatError::AmbiguousConstruct {
            context,
            snippet: snippet_from_elements(elements),
        });
    }

    let is_token_braced = matches!(
        significant.first(),
        Some(NodeOrToken::Token(tok)) if tok.kind() == SyntaxKind::LBRACE
    ) && matches!(
        significant.last(),
        Some(NodeOrToken::Token(tok)) if tok.kind() == SyntaxKind::RBRACE
    );
    if !is_token_braced {
        return match format_expr_with_optional_comment(elements, context, indent, ctx) {
            Ok(formatted) => Ok(formatted),
            Err(FormatError::AmbiguousConstruct { .. }) => {
                format_expr_tokens_via_parser(elements, context, indent, ctx)
            }
            Err(err) => Err(err),
        };
    }

    if significant.len() == 2 {
        return Ok("{}".to_string());
    }

    let inner = &significant[1..significant.len() - 1];
    let inner_text = format_expr_with_optional_comment(inner, context, indent + 1, ctx)?;
    Ok(format!(
        "{{\n{}{}\n{}}}",
        ctx.indent_text(indent + 1),
        inner_text,
        ctx.indent_text(indent)
    ))
}

fn format_expr_tokens_via_parser(
    elements: &[SyntaxElement<RLanguage>],
    context: &'static str,
    indent: usize,
    ctx: FormatContext,
) -> Result<String, FormatError> {
    let snippet = snippet_from_elements(elements);
    let parsed = parse(&snippet);
    if !parsed.diagnostics.is_empty() {
        return Err(FormatError::AmbiguousConstruct { context, snippet });
    }
    let significant: Vec<_> = parsed
        .cst
        .children_with_tokens()
        .filter(|el| !super::super::core::is_trivia(el.kind()))
        .collect();
    if significant.len() != 1 {
        return Err(FormatError::AmbiguousConstruct { context, snippet });
    }
    format_expr_element(&significant[0], indent, ctx)
}

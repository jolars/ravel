use rowan::{NodeOrToken, SyntaxElement};

use super::super::context::FormatContext;
use super::super::core::{
    FormatError, format_expr_element, format_expr_segment, format_expr_with_optional_comment,
    ir_block_expr_with_prefixed_comments, ir_expr_element, ir_expr_segment,
    ir_expr_with_optional_comment, ir_line, snippet_from_elements,
};
use super::super::ir::Ir;
use super::super::trivia::split_lines;
use super::expressions::{
    ArgSlot, build_arg_group, build_arg_hug, build_arg_hug_conditional, expr_ends_in_block,
    should_force_leading_hole_expand,
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

// ============================== Native IR call ==============================
//
// `ir_call_expr` renders `callee(args)` natively on the IR, reusing the shared
// arg-list machinery (holes, separators, trailing-block hug) from `expressions`.
// Calls whose args contain comments or function definitions fall back to the
// legacy string renderer: comment relocation and function-body autobracing /
// the trailing-function hug are migrated separately.

pub(crate) fn ir_call_expr(
    node: &SyntaxNode,
    indent: usize,
    ctx: FormatContext,
) -> Result<Ir, FormatError> {
    let elements: Vec<_> = node.children_with_tokens().collect();
    let lparen_idx = elements
        .iter()
        .position(|el| matches!(el, NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::LPAREN))
        .ok_or_else(|| FormatError::AmbiguousConstruct {
            context: "missing '(' in call expression",
            snippet: node.text().to_string(),
        })?;
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

    if call_comment_path_unsupported(&arg_list) {
        return Ok(Ir::verbatim(format_call_expr(node, indent, ctx)?));
    }

    let callee = ir_expr_segment(&elements[..lparen_idx], "call callee", indent, ctx)?;

    // Comments are relocated natively (own-line vs trailing) by an always-broken
    // item-stream layout; the flat/hug optimizations below never apply once a
    // comment is present.
    if arg_list_needs_comment_layout(&arg_list) {
        return Ok(Ir::concat([
            callee,
            ir_call_args_with_comments(&arg_list, indent, ctx)?,
        ]));
    }

    let (slots, comma_count) = collect_call_ir_slots(&arg_list, indent, ctx)?;

    // Empty call: no arguments and no holes.
    if comma_count == 0 && slots.iter().all(ArgSlot::is_empty_hole) {
        return Ok(Ir::concat([callee, Ir::text("()")]));
    }

    // Single-argument hug: a lone positional argument that owns a breakable
    // trailing arg list (a call/subset, possibly behind `::`/`$`) hugs the
    // parens with no extra indent, so nested wrapping falls out of the inner
    // construct's own group (`c(list(\n  ...\n))`, `abort(glue::glue(\n  ...\n))`).
    if comma_count == 0
        && let [ArgSlot::Expr { ir, .. }] = slots.as_slice()
        && single_arg_is_huggable(&arg_list)
    {
        return Ok(Ir::concat([
            callee,
            Ir::text("("),
            ir.clone(),
            Ir::text(")"),
        ]));
    }

    let force_named_functions = should_force_multiline_named_functions(&arg_list);
    Ok(Ir::concat([
        callee,
        build_call_args_ir(&slots, force_named_functions),
    ]))
}

/// The native port of [`should_force_multiline_for_named_function_args`]: a call
/// with more than one non-empty argument and at least two named arguments whose
/// value is a `function` definition always expands one argument per line, even
/// when it would otherwise fit (`list(a = function() {}, b = function() {})`).
fn should_force_multiline_named_functions(arg_list: &SyntaxNode) -> bool {
    let args: Vec<_> = arg_list
        .children()
        .filter(|n| n.kind() == SyntaxKind::ARG)
        .collect();
    let non_empty = args.iter().filter(|arg| arg_has_significant(arg)).count();
    if non_empty <= 1 {
        return false;
    }
    args.iter().filter(|arg| arg_is_named_function(arg)).count() >= 2
}

fn arg_has_significant(arg: &SyntaxNode) -> bool {
    arg.children_with_tokens()
        .any(|el| !super::super::core::is_trivia(el.kind()))
}

/// A named argument whose value is (or contains) a `function` definition,
/// matching the legacy `is_named && formatted.contains("function(")` test.
/// Lambda (`\(...)`) values do not count, since they never render `function(`.
fn arg_is_named_function(arg: &SyntaxNode) -> bool {
    let is_named = arg
        .children_with_tokens()
        .any(|el| matches!(el, NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::ASSIGN_EQ));
    is_named
        && arg.descendants().any(|n| {
            n.kind() == SyntaxKind::FUNCTION_EXPR
                && n.children_with_tokens().any(|el| {
                    matches!(el, NodeOrToken::Token(tok)
                        if tok.kind() == SyntaxKind::FUNCTION_KW && tok.text() == "function")
                })
        })
}

/// The one comment shape the native comment layout does not reproduce: an
/// argument that `format_arg` routes to the `ASSIGNMENT_EXPR` string helpers
/// (`format_assignment_expr_arg` / `format_named_arg_with_assignment_node`) *and*
/// that carries a comment inside the assignment. Such args are not producible
/// from diagnostic-free input today, but the gate keeps them on the legacy
/// renderer rather than dropping the comment.
fn call_comment_path_unsupported(arg_list: &SyntaxNode) -> bool {
    arg_list
        .children()
        .filter(|n| n.kind() == SyntaxKind::ARG)
        .any(|arg| {
            let significant: Vec<_> = arg
                .children_with_tokens()
                .filter(|el| !super::super::core::is_trivia(el.kind()))
                .collect();
            let assignment_based = match significant.as_slice() {
                [NodeOrToken::Node(n)] => n.kind() == SyntaxKind::ASSIGNMENT_EXPR,
                [NodeOrToken::Token(t), NodeOrToken::Node(n)] => {
                    t.kind() == SyntaxKind::IDENT && n.kind() == SyntaxKind::ASSIGNMENT_EXPR
                }
                _ => false,
            };
            assignment_based
                && arg
                    .descendants_with_tokens()
                    .any(|el| el.kind() == SyntaxKind::COMMENT)
        })
}

/// Whether this arg list owns comments that need relocating onto their own lines
/// or trailing the previous argument. That is the case for a comment sitting
/// directly in an `ARG` (a comment-only arg, or a leading/trailing/around-`=`
/// comment) or for a curly-curly `{{ … }}` argument whose lifted comments this
/// level prints. Comments buried in a nested call/subset/function/block are
/// relocated by that construct's own renderer, so they do not count here.
fn arg_list_needs_comment_layout(arg_list: &SyntaxNode) -> bool {
    arg_list
        .children()
        .filter(|n| n.kind() == SyntaxKind::ARG)
        .any(|arg| {
            arg.children_with_tokens()
                .any(|el| el.kind() == SyntaxKind::COMMENT)
                || arg_value_is_commented_curly_curly(&arg)
        })
}

/// Whether an argument's value is a curly-curly `{{ … }}` carrying a comment
/// somewhere inside (so its comments must be lifted out — see
/// [`ir_curly_curly_with_comments`]). Handles both bare (`{{ x }}`) and named
/// (`name = {{ x }}`) arguments.
fn arg_value_is_commented_curly_curly(arg: &SyntaxNode) -> bool {
    let elements: Vec<_> = arg.children_with_tokens().collect();
    let value_elements = match elements
        .iter()
        .position(|el| matches!(el, NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::ASSIGN_EQ))
    {
        Some(eq_idx) => &elements[eq_idx + 1..],
        None => &elements[..],
    };
    let value_significant: Vec<_> = value_elements
        .iter()
        .filter(|el| !super::super::core::is_trivia(el.kind()))
        .cloned()
        .collect();
    let [NodeOrToken::Node(outer)] = value_significant.as_slice() else {
        return false;
    };
    is_curly_curly_symbol_block(outer)
        && outer
            .descendants_with_tokens()
            .any(|el| el.kind() == SyntaxKind::COMMENT)
}

/// Whether a `BLOCK_EXPR` is a curly-curly `{{ symbol }}` wrapper that
/// [`ir_curly_curly_with_comments`] will accept: an outer block whose only
/// significant content (ignoring comments) is an inner block holding exactly one
/// `IDENT`. A `{{ … }}` with zero or a non-symbol inner expression is *not* a
/// curly-curly — it renders as ordinary nested blocks (and so must not be routed
/// to the comment layout, where it would lose the trailing-block hug).
fn is_curly_curly_symbol_block(outer: &SyntaxNode) -> bool {
    if outer.kind() != SyntaxKind::BLOCK_EXPR {
        return false;
    }
    let outer_sig: Vec<_> = outer
        .children_with_tokens()
        .filter(|el| !super::super::core::is_trivia(el.kind()))
        .collect();
    if outer_sig.len() < 2 {
        return false;
    }
    let (Some(NodeOrToken::Token(l)), Some(NodeOrToken::Token(r))) =
        (outer_sig.first(), outer_sig.last())
    else {
        return false;
    };
    if l.kind() != SyntaxKind::LBRACE || r.kind() != SyntaxKind::RBRACE {
        return false;
    }
    let mut inner = None::<SyntaxNode>;
    for el in &outer_sig[1..outer_sig.len() - 1] {
        match el {
            NodeOrToken::Node(n) if n.kind() == SyntaxKind::BLOCK_EXPR => {
                if inner.is_some() {
                    return false;
                }
                inner = Some(n.clone());
            }
            NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::COMMENT => {}
            _ => return false,
        }
    }
    let Some(inner) = inner else {
        return false;
    };
    let inner_sig: Vec<_> = inner
        .children_with_tokens()
        .filter(|el| !super::super::core::is_trivia(el.kind()))
        .collect();
    if inner_sig.len() < 3 {
        return false;
    }
    let (Some(NodeOrToken::Token(il)), Some(NodeOrToken::Token(ir))) =
        (inner_sig.first(), inner_sig.last())
    else {
        return false;
    };
    if il.kind() != SyntaxKind::LBRACE || ir.kind() != SyntaxKind::RBRACE {
        return false;
    }
    let exprs: Vec<_> = inner_sig[1..inner_sig.len() - 1]
        .iter()
        .filter(|el| !matches!(el, NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::COMMENT))
        .collect();
    matches!(exprs.as_slice(), [NodeOrToken::Token(tok)] if tok.kind() == SyntaxKind::IDENT)
}

/// A *positional* function-definition argument (`function(...) ...` or
/// `\(...) ...`) used as the trailing arg of a call. Named function args
/// (`f = function(...) ...`) are excluded, mirroring the legacy renderer,
/// which only hugs trailing args that start with `function(`.
fn expr_is_positional_function(node: &SyntaxNode) -> bool {
    node.kind() == SyntaxKind::FUNCTION_EXPR && !value_node_is_named_arg(node)
}

/// Whether this argument-value node sits in a named call argument: its parent is
/// an `ARG` carrying an `=` (`name = <node>`).
fn value_node_is_named_arg(node: &SyntaxNode) -> bool {
    node.parent().is_some_and(|arg| {
        arg.kind() == SyntaxKind::ARG
            && arg.children_with_tokens().any(
                |el| matches!(el, NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::ASSIGN_EQ),
            )
    })
}

/// Native IR for a curly-curly `{{ x }}` call argument. Flat: `{{ x }}`; when the
/// symbol can't fit inline the group breaks to put it on its own indented line.
/// Returns `None` for any other shape (`{{ 1 }}`, `{{ (x) }}`, multi-statement,
/// ...), which then formats as ordinary nested blocks. Comment-bearing forms
/// never reach here — they route to the legacy renderer via [`call_needs_legacy`].
fn ir_curly_curly(
    significant: &[SyntaxElement<RLanguage>],
    indent: usize,
    ctx: FormatContext,
) -> Result<Option<Ir>, FormatError> {
    let Some(symbol) = curly_curly_inner_symbol(significant) else {
        return Ok(None);
    };
    let inner = ir_expr_element(&symbol, indent + 1, ctx)?;
    Ok(Some(Ir::group(Ir::concat([
        Ir::text("{{"),
        Ir::indent(Ir::concat([Ir::line(), inner])),
        Ir::line(),
        Ir::text("}}"),
    ]))))
}

/// The single symbol of a curly-curly `{{ x }}` argument: an outer `BLOCK_EXPR`
/// wrapping an inner `BLOCK_EXPR` whose only content is one identifier. Returns
/// `None` for any other shape, matching the legacy renderer's symbol-only
/// detection.
fn curly_curly_inner_symbol(
    significant: &[SyntaxElement<RLanguage>],
) -> Option<SyntaxElement<RLanguage>> {
    let [NodeOrToken::Node(outer)] = significant else {
        return None;
    };
    if outer.kind() != SyntaxKind::BLOCK_EXPR {
        return None;
    }
    let outer_significant: Vec<_> = outer
        .children_with_tokens()
        .filter(|el| !super::super::core::is_trivia(el.kind()))
        .collect();
    let [
        NodeOrToken::Token(outer_l),
        NodeOrToken::Node(inner),
        NodeOrToken::Token(outer_r),
    ] = outer_significant.as_slice()
    else {
        return None;
    };
    if outer_l.kind() != SyntaxKind::LBRACE
        || outer_r.kind() != SyntaxKind::RBRACE
        || inner.kind() != SyntaxKind::BLOCK_EXPR
    {
        return None;
    }
    let inner_significant: Vec<_> = inner
        .children_with_tokens()
        .filter(|el| !super::super::core::is_trivia(el.kind()))
        .collect();
    let [
        NodeOrToken::Token(inner_l),
        inner_expr @ NodeOrToken::Token(symbol),
        NodeOrToken::Token(inner_r),
    ] = inner_significant.as_slice()
    else {
        return None;
    };
    if inner_l.kind() == SyntaxKind::LBRACE
        && inner_r.kind() == SyntaxKind::RBRACE
        && symbol.kind() == SyntaxKind::IDENT
    {
        Some(inner_expr.clone())
    } else {
        None
    }
}

/// Whether a call's sole argument is a lone positional expression that owns a
/// non-empty trailing arg list, so the call can hug it with no extra indent.
fn single_arg_is_huggable(arg_list: &SyntaxNode) -> bool {
    let args: Vec<_> = arg_list
        .children()
        .filter(|n| n.kind() == SyntaxKind::ARG)
        .collect();
    let [arg] = args.as_slice() else {
        return false;
    };
    // Named arguments (`name = value`) never hug.
    if arg
        .children_with_tokens()
        .any(|el| matches!(el, NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::ASSIGN_EQ))
    {
        return false;
    }
    let significant: Vec<_> = arg
        .children_with_tokens()
        .filter(|el| !super::super::core::is_trivia(el.kind()))
        .collect();
    let [NodeOrToken::Node(n)] = significant.as_slice() else {
        return false;
    };
    is_huggable_node(n)
}

/// A call/subset with a non-empty arg list, or a `::`/`$`-style binary whose
/// right side is one — i.e. it ends in a bracketed list that can break.
fn is_huggable_node(node: &SyntaxNode) -> bool {
    match node.kind() {
        SyntaxKind::CALL_EXPR | SyntaxKind::SUBSET_EXPR | SyntaxKind::SUBSET2_EXPR => {
            call_or_subset_has_content(node)
        }
        // A sole function-definition argument hugs the parens with no extra
        // indent and breaks its own params / braces its own body
        // (`fn(function(<long params>) { ... })`).
        SyntaxKind::FUNCTION_EXPR => true,
        SyntaxKind::BINARY_EXPR => node
            .children()
            .last()
            .is_some_and(|rhs| is_huggable_node(&rhs)),
        _ => false,
    }
}

fn call_or_subset_has_content(node: &SyntaxNode) -> bool {
    node.children()
        .find(|c| c.kind() == SyntaxKind::ARG_LIST)
        .is_some_and(|arg_list| {
            arg_list.children_with_tokens().any(|el| match el {
                NodeOrToken::Token(tok) => tok.kind() == SyntaxKind::COMMA,
                NodeOrToken::Node(arg) => {
                    arg.kind() == SyntaxKind::ARG
                        && arg
                            .children_with_tokens()
                            .any(|e| !super::super::core::is_trivia(e.kind()))
                }
            })
        })
}

fn collect_call_ir_slots(
    arg_list: &SyntaxNode,
    indent: usize,
    ctx: FormatContext,
) -> Result<(Vec<ArgSlot>, usize), FormatError> {
    let mut slots: Vec<ArgSlot> = Vec::new();
    let mut comma_count = 0usize;
    let mut current: Option<ArgSlot> = None;
    for element in arg_list.children_with_tokens() {
        match element {
            NodeOrToken::Node(arg) if arg.kind() == SyntaxKind::ARG => {
                let arg_elements: Vec<_> = arg.children_with_tokens().collect();
                let significant: Vec<_> = arg_elements
                    .iter()
                    .filter(|el| !super::super::core::is_trivia(el.kind()))
                    .cloned()
                    .collect();
                if significant.is_empty() {
                    continue;
                }
                current = Some(ir_call_argument(&arg_elements, &significant, indent, ctx)?);
            }
            NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::COMMA => {
                slots.push(current.take().unwrap_or(ArgSlot::Empty));
                comma_count += 1;
            }
            _ => {}
        }
    }
    slots.push(current.take().unwrap_or(ArgSlot::Empty));
    Ok((slots, comma_count))
}

fn ir_call_argument(
    elements: &[SyntaxElement<RLanguage>],
    significant: &[SyntaxElement<RLanguage>],
    indent: usize,
    ctx: FormatContext,
) -> Result<ArgSlot, FormatError> {
    // Named argument `name = value`: in calls these are raw tokens (not an
    // `ASSIGNMENT_EXPR` node as in subsets), so split on `=`. `expr_node` points
    // at the value so a `name = { ... }` arg is still seen as a trailing block.
    if let Some(eq_idx) = elements
        .iter()
        .position(|el| matches!(el, NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::ASSIGN_EQ))
    {
        let (name_ir, name_empty) =
            ir_arg_side(&elements[..eq_idx], "named arg name", indent, ctx)?;
        let value_elements = &elements[eq_idx + 1..];
        let value_significant: Vec<_> = value_elements
            .iter()
            .filter(|el| !super::super::core::is_trivia(el.kind()))
            .cloned()
            .collect();
        let value_node = single_node(&value_significant);
        let value_ir = if value_significant.is_empty() {
            None
        } else if let Some(curly) = ir_curly_curly(&value_significant, indent, ctx)? {
            Some(curly)
        } else {
            Some(ir_expr_segment(
                value_elements,
                "named arg value",
                indent,
                ctx,
            )?)
        };
        let ir = build_named_arg_ir(name_ir, name_empty, value_ir);
        return Ok(ArgSlot::Expr {
            ir,
            expr_node: value_node,
        });
    }

    let expr_node = single_node(significant);
    // Curly-curly `{{ x }}` renders natively as its own group.
    if let Some(curly) = ir_curly_curly(significant, indent, ctx)? {
        return Ok(ArgSlot::Expr {
            ir: curly,
            expr_node,
        });
    }
    let ir = ir_expr_segment(elements, "call argument", indent, ctx)?;
    Ok(ArgSlot::Expr { ir, expr_node })
}

fn single_node(significant: &[SyntaxElement<RLanguage>]) -> Option<SyntaxNode> {
    match significant {
        [NodeOrToken::Node(n)] => Some(n.clone()),
        _ => None,
    }
}

/// Format one side of a named argument; reports whether it was empty.
fn ir_arg_side(
    elements: &[SyntaxElement<RLanguage>],
    context: &'static str,
    indent: usize,
    ctx: FormatContext,
) -> Result<(Ir, bool), FormatError> {
    let has_significant = elements
        .iter()
        .any(|el| !super::super::core::is_trivia(el.kind()));
    if !has_significant {
        return Ok((Ir::nil(), true));
    }
    Ok((ir_expr_segment(elements, context, indent, ctx)?, false))
}

/// `name = value`, with the legacy spacing for the value-less variants
/// (`name =`, `= value`, `=`).
fn build_named_arg_ir(name_ir: Ir, name_empty: bool, value_ir: Option<Ir>) -> Ir {
    match (name_empty, value_ir) {
        (false, Some(value)) => Ir::concat([name_ir, Ir::text(" = "), value]),
        (false, None) => Ir::concat([name_ir, Ir::text(" =")]),
        (true, Some(value)) => Ir::concat([Ir::text("= "), value]),
        (true, None) => Ir::text("="),
    }
}

fn build_call_args_ir(slots: &[ArgSlot], force_named_functions: bool) -> Ir {
    let last = slots.len() - 1;
    let first_non_empty = slots.iter().position(|s| !s.is_empty_hole());
    let no_non_empty = first_non_empty.is_none();

    // A positional trailing function-definition argument hugs its call: the
    // call's first line `callee(leading, function(` must fit, and any further
    // breaking happens inside the function. This is the IR-native form of the
    // legacy "format-then-measure" hug. Route it through the conditional
    // variant whose break-aware first-line measurement lets the function's
    // own params/body group break naturally during the decision (the flat
    // `group_hug` would measure the function flat and overflow before its
    // params have a chance to break, collapsing the whole call into the
    // expanded one-arg-per-line layout). A plain trailing block has no such
    // nested breakable group before its opening brace, so the flat-only
    // `group_hug` still suffices.
    let leading_ok = !force_named_functions && slots[..last].iter().all(|s| !s.has_forced_break());
    let trailing_function = leading_ok
        && matches!(&slots[last], ArgSlot::Expr { expr_node: Some(node), .. }
            if expr_is_positional_function(node));
    let trailing_block = leading_ok
        && matches!(&slots[last], ArgSlot::Expr { ir, expr_node: Some(node) }
            if expr_ends_in_block(node) && ir.contains_forced_break());
    if trailing_function {
        return build_arg_hug_conditional(slots, "(", ")", first_non_empty, no_non_empty);
    }
    if trailing_block {
        return build_arg_hug(slots, "(", ")", first_non_empty, no_non_empty);
    }

    let leading_hole = slots[0].is_empty_hole();
    let force = force_named_functions || should_force_leading_hole_expand(slots, first_non_empty);
    let hug_leading_hole = force && leading_hole && !force_named_functions;
    build_arg_group(
        slots,
        "(",
        ")",
        first_non_empty,
        no_non_empty,
        force,
        hug_leading_hole,
    )
}

// ===================== Native IR call comment relocation =====================
//
// When an arg list owns comments, the layout is *always* broken: the flat and
// hug forms never apply. `ir_call_args_with_comments` walks a flat item stream
// (`Arg` / `Comma`, the IR port of `collect_call_items`) and decides, per
// comment, whether it trails the previous argument's last line, leads the next
// element on its own line, or stands alone — reproducing `format_arg_list_multiline`
// while emitting real IR for every argument expression.

/// One formatted argument in the comment-aware item stream.
struct IrCallArg {
    /// The argument's IR (for a comment-only arg, just the comment text).
    ir: Ir,
    /// An empty hole, e.g. the gaps in `f(, a)`.
    is_empty: bool,
    /// A comment-only arg (`# note` with no expression).
    is_comment_only: bool,
    /// The raw comment text, for a comment-only arg.
    comment_text: String,
    /// Whether a source newline precedes this arg (distinguishes a comment that
    /// trails the previous line from one that leads on its own line).
    leading_newline: bool,
    /// Whether the rendered argument ends in `=` (a value-less named arg), which
    /// changes the comma/comment separator (` ,` / ` , ` rather than `,` / `, `).
    ends_with_eq: bool,
}

enum IrCallItem {
    Arg(IrCallArg),
    Comma { newline_after: bool },
}

fn ir_call_args_with_comments(
    arg_list: &SyntaxNode,
    indent: usize,
    ctx: FormatContext,
) -> Result<Ir, FormatError> {
    let items = collect_call_comment_items(arg_list, indent, ctx)?;
    let lines = layout_call_comment_items(&items);
    if lines.is_empty() {
        return Ok(Ir::text("()"));
    }
    Ok(Ir::concat([
        Ir::text("("),
        Ir::indent(Ir::concat([
            Ir::hard_line(),
            Ir::join(Ir::hard_line(), lines),
        ])),
        Ir::hard_line(),
        Ir::text(")"),
    ]))
}

fn collect_call_comment_items(
    arg_list: &SyntaxNode,
    indent: usize,
    ctx: FormatContext,
) -> Result<Vec<IrCallItem>, FormatError> {
    let elements: Vec<_> = arg_list.children_with_tokens().collect();
    let mut items = Vec::new();
    for (idx, element) in elements.iter().enumerate() {
        match element {
            NodeOrToken::Node(arg) if arg.kind() == SyntaxKind::ARG => {
                let mut arg_info = ir_call_comment_arg(arg, indent, ctx)?;
                arg_info.leading_newline = has_newline_before_arg(&elements, idx);
                items.push(IrCallItem::Arg(arg_info));
            }
            NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::COMMA => {
                items.push(IrCallItem::Comma {
                    newline_after: comma_followed_by_newline(&elements, idx),
                });
            }
            _ => {}
        }
    }
    Ok(items)
}

/// Whether a comma at `idx` is followed by a newline before the next argument or
/// comma. Mirrors the `newline_after` scan in `collect_call_items`.
fn comma_followed_by_newline(elements: &[SyntaxElement<RLanguage>], idx: usize) -> bool {
    for next in elements.iter().skip(idx + 1) {
        match next {
            NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::NEWLINE => return true,
            NodeOrToken::Token(tok)
                if tok.kind() == SyntaxKind::WHITESPACE || tok.kind() == SyntaxKind::COMMENT => {}
            NodeOrToken::Node(n) if n.kind() == SyntaxKind::ARG => return false,
            NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::COMMA => return false,
            _ => return false,
        }
    }
    false
}

/// IR port of [`format_arg`] for the comment layout: classifies the arg and
/// builds its IR (with any leading/internal comments lifted onto their own
/// lines).
fn ir_call_comment_arg(
    arg: &SyntaxNode,
    indent: usize,
    ctx: FormatContext,
) -> Result<IrCallArg, FormatError> {
    let elements: Vec<_> = arg.children_with_tokens().collect();
    let significant: Vec<_> = elements
        .iter()
        .filter(|el| !super::super::core::is_trivia(el.kind()))
        .cloned()
        .collect();
    if significant.is_empty() {
        return Ok(IrCallArg {
            ir: Ir::nil(),
            is_empty: true,
            is_comment_only: false,
            comment_text: String::new(),
            leading_newline: false,
            ends_with_eq: false,
        });
    }
    if let [NodeOrToken::Token(tok)] = significant.as_slice()
        && tok.kind() == SyntaxKind::COMMENT
    {
        let text = tok.text().to_string();
        return Ok(IrCallArg {
            ir: Ir::text(text.clone()),
            is_empty: false,
            is_comment_only: true,
            comment_text: text,
            leading_newline: false,
            ends_with_eq: false,
        });
    }

    let (ir, ends_with_eq) = ir_call_arg_value(&elements, &significant, indent, ctx)?;
    Ok(IrCallArg {
        ir,
        is_empty: false,
        is_comment_only: false,
        comment_text: String::new(),
        leading_newline: false,
        ends_with_eq,
    })
}

/// Build the IR for a non-comment-only argument value, plus whether it ends in
/// `=`. Mirrors the curly-curly / named-arg / positional branches of
/// [`format_arg`], lifting any leading/around-`=` comments onto their own lines.
fn ir_call_arg_value(
    elements: &[SyntaxElement<RLanguage>],
    significant: &[SyntaxElement<RLanguage>],
    indent: usize,
    ctx: FormatContext,
) -> Result<(Ir, bool), FormatError> {
    if let Some(curly) = ir_curly_curly_with_comments(significant, indent, ctx)? {
        return Ok((curly, false));
    }

    if let Some(eq_idx) = elements
        .iter()
        .position(|el| matches!(el, NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::ASSIGN_EQ))
    {
        let lhs_comments: Vec<String> = elements[..eq_idx]
            .iter()
            .filter_map(comment_text_of)
            .collect();
        let lhs_significant: Vec<_> = elements[..eq_idx]
            .iter()
            .filter(|el| {
                !super::super::core::is_trivia(el.kind()) && el.kind() != SyntaxKind::COMMENT
            })
            .cloned()
            .collect();
        let name_empty = lhs_significant.is_empty();
        let name_ir = if name_empty {
            Ir::nil()
        } else {
            ir_expr_segment(&lhs_significant, "named arg name", indent, ctx)?
        };
        let (rhs_comments, value_ir) =
            ir_rhs_with_leading_comments(&elements[eq_idx + 1..], indent, ctx)?;
        let value_empty = value_ir.is_none();
        let base = build_named_arg_ir(name_ir, name_empty, value_ir);
        let mut comments = lhs_comments;
        comments.extend(rhs_comments);
        return Ok((prepend_comment_lines(&comments, base), value_empty));
    }

    let ir = ir_expr_with_optional_comment(elements, "positional arg", indent, ctx)?;
    Ok((ir, false))
}

/// IR port of [`format_assignment_rhs_with_leading_comments`]: peel any leading
/// comments off the value, then build the (optional) value IR with a trailing
/// comment honored.
fn ir_rhs_with_leading_comments(
    elements: &[SyntaxElement<RLanguage>],
    indent: usize,
    ctx: FormatContext,
) -> Result<(Vec<String>, Option<Ir>), FormatError> {
    let mut idx = 0usize;
    let mut leading = Vec::new();
    while idx < elements.len() {
        match &elements[idx] {
            NodeOrToken::Token(tok) if super::super::core::is_trivia(tok.kind()) => idx += 1,
            NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::COMMENT => {
                leading.push(tok.text().to_string());
                idx += 1;
            }
            _ => break,
        }
    }
    if idx >= elements.len() {
        return Ok((leading, None));
    }
    // A curly-curly `{{ x }}` value is lifted natively, matching the no-comment
    // named-arg path (`ir_call_argument`); other values render as one expression
    // with an optional trailing comment.
    let value_significant: Vec<_> = elements[idx..]
        .iter()
        .filter(|el| !super::super::core::is_trivia(el.kind()))
        .cloned()
        .collect();
    let value = if let Some(curly) = ir_curly_curly_with_comments(&value_significant, indent, ctx)?
    {
        curly
    } else {
        ir_expr_with_optional_comment(&elements[idx..], "assignment rhs", indent, ctx)?
    };
    Ok((leading, Some(value)))
}

fn comment_text_of(el: &SyntaxElement<RLanguage>) -> Option<String> {
    match el {
        NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::COMMENT => {
            Some(tok.text().to_string())
        }
        _ => None,
    }
}

/// Prefix `base` with one own-line comment per entry (`# c\n# d\nbase`).
fn prepend_comment_lines(comments: &[String], base: Ir) -> Ir {
    if comments.is_empty() {
        return base;
    }
    let mut parts: Vec<Ir> = Vec::new();
    for comment in comments {
        parts.push(Ir::verbatim_forced(comment.clone()));
        parts.push(Ir::hard_line());
    }
    parts.push(base);
    Ir::concat(parts)
}

/// IR port of [`format_arg_list_multiline`]: turn the item stream into one IR per
/// output line, deciding comment placement. Always emits a fully-broken list.
fn layout_call_comment_items(items: &[IrCallItem]) -> Vec<Ir> {
    let mut out: Vec<Ir> = Vec::new();
    let mut i = 0usize;
    while i < items.len() {
        match &items[i] {
            IrCallItem::Arg(arg) if arg.is_empty => {
                i += 1;
            }
            IrCallItem::Arg(arg) if arg.is_comment_only => {
                out.push(Ir::verbatim_forced(arg.comment_text.clone()));
                i += 1;
            }
            IrCallItem::Arg(arg) => {
                // A comment-only arg directly after this one (no comma between)
                // and on the same source line trails the argument's last line.
                if let Some(IrCallItem::Arg(comment_arg)) = items.get(i + 1)
                    && comment_arg.is_comment_only
                    && !comment_arg.leading_newline
                {
                    out.push(Ir::concat([
                        arg.ir.clone(),
                        Ir::text(" "),
                        Ir::text(comment_arg.comment_text.clone()),
                    ]));
                    i += 2;
                    continue;
                }

                // `arg, # comment` — the comment shares the comma's line. Trailing
                // comment-only args after it align under the comment (` ` ×3).
                if let (
                    Some(IrCallItem::Comma {
                        newline_after: false,
                    }),
                    Some(IrCallItem::Arg(comment_arg)),
                ) = (items.get(i + 1), items.get(i + 2))
                    && comment_arg.is_comment_only
                {
                    let sep = if arg.ends_with_eq { " , " } else { ", " };
                    out.push(Ir::concat([
                        arg.ir.clone(),
                        Ir::text(sep),
                        Ir::text(comment_arg.comment_text.clone()),
                    ]));
                    i += 3;
                    while let Some(IrCallItem::Arg(extra)) = items.get(i) {
                        if !extra.is_comment_only {
                            break;
                        }
                        out.push(Ir::text(format!("   {}", extra.comment_text)));
                        i += 1;
                    }
                    continue;
                }

                if matches!(items.get(i + 1), Some(IrCallItem::Comma { .. })) {
                    out.push(Ir::concat([
                        arg.ir.clone(),
                        Ir::text(if arg.ends_with_eq { " ," } else { "," }),
                    ]));
                    i += 2;
                } else {
                    out.push(arg.ir.clone());
                    i += 1;
                }
            }
            IrCallItem::Comma { .. } => {
                out.push(Ir::text(","));
                i += 1;
            }
        }
    }
    out
}

/// IR port of [`try_format_curly_curly`]. Returns `Some` when `significant` is a
/// curly-curly `{{ symbol }}` wrapper; comments are lifted out and placed
/// (leading the `{{`, leading/trailing the symbol, trailing the `}}`, or below
/// it) exactly as the legacy renderer does. With no comments, defers to the flat
/// [`ir_curly_curly`] group. Returns `None` for any non-curly-curly shape.
fn ir_curly_curly_with_comments(
    significant: &[SyntaxElement<RLanguage>],
    indent: usize,
    ctx: FormatContext,
) -> Result<Option<Ir>, FormatError> {
    let [NodeOrToken::Node(outer)] = significant else {
        return Ok(None);
    };
    if outer.kind() != SyntaxKind::BLOCK_EXPR {
        return Ok(None);
    }
    let outer_elements: Vec<_> = outer.children_with_tokens().collect();
    let outer_significant: Vec<_> = outer_elements
        .iter()
        .filter(|el| !super::super::core::is_trivia(el.kind()))
        .cloned()
        .collect();
    let (Some(NodeOrToken::Token(outer_l)), Some(NodeOrToken::Token(outer_r))) =
        (outer_significant.first(), outer_significant.last())
    else {
        return Ok(None);
    };
    if outer_l.kind() != SyntaxKind::LBRACE || outer_r.kind() != SyntaxKind::RBRACE {
        return Ok(None);
    }

    let mut inner_node = None::<SyntaxNode>;
    let mut outer_leading_comments = Vec::new();
    for element in outer_significant
        .iter()
        .skip(1)
        .take(outer_significant.len().saturating_sub(2))
    {
        match element {
            NodeOrToken::Node(node) if node.kind() == SyntaxKind::BLOCK_EXPR => {
                if inner_node.is_some() {
                    return Ok(None);
                }
                inner_node = Some(node.clone());
            }
            NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::COMMENT => {
                if inner_node.is_some() {
                    break;
                }
                outer_leading_comments.push(tok.text().to_string());
            }
            _ => return Ok(None),
        }
    }
    let Some(inner) = inner_node else {
        return Ok(None);
    };

    let inner_significant: Vec<_> = inner
        .children_with_tokens()
        .filter(|el| !super::super::core::is_trivia(el.kind()))
        .collect();
    if inner_significant.len() < 3 {
        return Ok(None);
    }
    let (Some(NodeOrToken::Token(inner_l)), Some(NodeOrToken::Token(inner_r))) =
        (inner_significant.first(), inner_significant.last())
    else {
        return Ok(None);
    };
    if inner_l.kind() != SyntaxKind::LBRACE || inner_r.kind() != SyntaxKind::RBRACE {
        return Ok(None);
    }

    let inner_payload = &inner_significant[1..inner_significant.len() - 1];
    let expr_count = inner_payload
        .iter()
        .filter(|el| !matches!(el, NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::COMMENT))
        .count();
    if expr_count != 1 {
        return Ok(None);
    }

    let mut inner_pre_comments = Vec::new();
    let mut inner_after_comments = Vec::new();
    let mut inner_expr = None::<SyntaxElement<RLanguage>>;
    for element in inner_payload {
        match element {
            NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::COMMENT => {
                if inner_expr.is_some() {
                    inner_after_comments.push(tok.text().to_string());
                } else {
                    inner_pre_comments.push(tok.text().to_string());
                }
            }
            _ => {
                if inner_expr.is_some() {
                    return Ok(None);
                }
                inner_expr = Some(element.clone());
            }
        }
    }
    let Some(inner_expr) = inner_expr else {
        return Ok(None);
    };
    if !matches!(&inner_expr, NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::IDENT) {
        return Ok(None);
    }

    let inner_idx = outer_elements
        .iter()
        .position(
            |el| matches!(el, NodeOrToken::Node(node) if node.kind() == SyntaxKind::BLOCK_EXPR),
        )
        .ok_or_else(|| FormatError::AmbiguousConstruct {
            context: "missing inner block for curly-curly",
            snippet: outer.text().to_string(),
        })?;
    let mut inline_trailing = None::<String>;
    let mut outer_post_comments = Vec::new();
    let mut saw_newline = false;
    for element in outer_elements.iter().skip(inner_idx + 1) {
        match element {
            NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::NEWLINE => saw_newline = true,
            NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::WHITESPACE => {}
            NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::COMMENT => {
                if !saw_newline && inline_trailing.is_none() {
                    inline_trailing = Some(tok.text().to_string());
                } else {
                    outer_post_comments.push(tok.text().to_string());
                }
            }
            NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::RBRACE => break,
            _ => return Ok(None),
        }
    }

    // No comments anywhere: defer to the flat/group curly-curly form.
    if outer_leading_comments.is_empty()
        && inner_pre_comments.is_empty()
        && inner_after_comments.is_empty()
        && outer_post_comments.is_empty()
        && inline_trailing.is_none()
    {
        return ir_curly_curly(significant, indent, ctx);
    }

    let expr_ir = ir_expr_element(&inner_expr, indent + 1, ctx)?;
    let mut parts: Vec<Ir> = Vec::new();
    for comment in &outer_leading_comments {
        parts.push(Ir::verbatim_forced(comment.clone()));
        parts.push(Ir::hard_line());
    }
    parts.push(Ir::text("{{"));
    let mut body: Vec<Ir> = Vec::new();
    for comment in &inner_pre_comments {
        body.push(Ir::hard_line());
        body.push(Ir::verbatim_forced(comment.clone()));
    }
    body.push(Ir::hard_line());
    body.push(expr_ir);
    for comment in &inner_after_comments {
        body.push(Ir::hard_line());
        body.push(Ir::verbatim_forced(comment.clone()));
    }
    parts.push(Ir::indent(Ir::concat(body)));
    parts.push(Ir::hard_line());
    parts.push(Ir::text("}}"));
    if let Some(comment) = inline_trailing {
        parts.push(Ir::text(" "));
        parts.push(Ir::text(comment));
    }
    for comment in &outer_post_comments {
        parts.push(Ir::hard_line());
        parts.push(Ir::verbatim_forced(comment.clone()));
    }
    Ok(Some(Ir::concat(parts)))
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
        && !formatted.contains("{{")
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
    let mut lines: Vec<String> = indent_multiline_arg(formatted, item_indent, false)
        .lines()
        .map(ToString::to_string)
        .collect();
    if lines.is_empty() {
        return format!("{item_indent}{comment}");
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
    if !looks_like_trailing_block_arg(&last.formatted) || last.formatted.starts_with("{{") {
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
    let outer_elements: Vec<_> = outer.children_with_tokens().collect();
    let outer_significant: Vec<_> = outer_elements
        .iter()
        .filter(|el| !super::super::core::is_trivia(el.kind()))
        .cloned()
        .collect();
    let Some(NodeOrToken::Token(outer_l)) = outer_significant.first() else {
        return Ok(None);
    };
    let Some(NodeOrToken::Token(outer_r)) = outer_significant.last() else {
        return Ok(None);
    };
    if outer_l.kind() != SyntaxKind::LBRACE || outer_r.kind() != SyntaxKind::RBRACE {
        return Ok(None);
    }

    let mut outer_inner_node = None::<SyntaxNode>;
    let mut outer_leading_comments = Vec::new();
    for element in outer_significant
        .iter()
        .skip(1)
        .take(outer_significant.len().saturating_sub(2))
    {
        match element {
            NodeOrToken::Node(node) if node.kind() == SyntaxKind::BLOCK_EXPR => {
                if outer_inner_node.is_some() {
                    return Ok(None);
                }
                outer_inner_node = Some(node.clone());
            }
            NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::COMMENT => {
                if outer_inner_node.is_some() {
                    break;
                }
                outer_leading_comments.push(tok.text().to_string());
            }
            _ => return Ok(None),
        }
    }
    let Some(inner) = outer_inner_node else {
        return Ok(None);
    };

    let inner_significant: Vec<_> = inner
        .children_with_tokens()
        .filter(|el| !super::super::core::is_trivia(el.kind()))
        .collect();
    if inner_significant.len() < 3 {
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

    let inner_payload = &inner_significant[1..inner_significant.len() - 1];
    let expr_count = inner_payload
        .iter()
        .filter(|el| !matches!(el, NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::COMMENT))
        .count();
    if expr_count != 1 {
        return Ok(None);
    }

    let mut inner_pre_comments = Vec::new();
    let mut inner_after_expr_comments = Vec::new();
    let mut inner_expr = None::<SyntaxElement<RLanguage>>;
    for element in inner_payload {
        match element {
            NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::COMMENT => {
                if inner_expr.is_some() {
                    inner_after_expr_comments.push(tok.text().to_string());
                } else {
                    inner_pre_comments.push(tok.text().to_string());
                }
            }
            _ => {
                if inner_expr.is_some() {
                    return Ok(None);
                }
                inner_expr = Some(element.clone());
            }
        }
    }
    let Some(inner_expr) = inner_expr else {
        return Ok(None);
    };
    if !matches!(&inner_expr, NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::IDENT) {
        return Ok(None);
    }
    let rendered_expr = format_expr_element(&inner_expr, indent + 1, ctx)?;

    let outer_inner_idx = outer_elements
        .iter()
        .position(
            |el| matches!(el, NodeOrToken::Node(node) if node.kind() == SyntaxKind::BLOCK_EXPR),
        )
        .ok_or_else(|| FormatError::AmbiguousConstruct {
            context: "missing inner block for curly-curly",
            snippet: outer.text().to_string(),
        })?;
    let mut inline_trailing_comment = None::<String>;
    let mut outer_post_comments = Vec::new();
    let mut saw_newline = false;
    for element in outer_elements.iter().skip(outer_inner_idx + 1) {
        match element {
            NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::NEWLINE => saw_newline = true,
            NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::WHITESPACE => {}
            NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::COMMENT => {
                if !saw_newline && inline_trailing_comment.is_none() {
                    inline_trailing_comment = Some(tok.text().to_string());
                } else {
                    outer_post_comments.push(tok.text().to_string());
                }
            }
            NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::RBRACE => break,
            _ => return Ok(None),
        }
    }

    if outer_leading_comments.is_empty()
        && inner_pre_comments.is_empty()
        && inner_after_expr_comments.is_empty()
        && outer_post_comments.is_empty()
        && inline_trailing_comment.is_none()
    {
        let inline = format!("{{{{ {rendered_expr} }}}}");
        if ctx.fits_inline(indent, &inline) {
            return Ok(Some(inline));
        }
    }

    let mut out_lines = Vec::new();
    out_lines.extend(outer_leading_comments);
    out_lines.push("{{".to_string());
    let inner_indent = ctx.indent_text(1);
    for comment in inner_pre_comments {
        out_lines.push(format!("{inner_indent}{comment}"));
    }
    for line in rendered_expr.lines() {
        out_lines.push(format!("{inner_indent}{line}"));
    }
    for comment in inner_after_expr_comments {
        out_lines.push(format!("{inner_indent}{comment}"));
    }
    let mut closing = "}}".to_string();
    if let Some(comment) = inline_trailing_comment {
        closing.push(' ');
        closing.push_str(&comment);
    }
    out_lines.push(closing);
    out_lines.extend(outer_post_comments);

    Ok(Some(out_lines.join("\n")))
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

// ============================ Native IR function ============================
//
// `ir_function_expr` renders `function(params) body` / `\(params) body`
// natively. The param list reuses the shared arg-group machinery; the body is
// chosen by the printer between a bare inline form (flat) and a braced block
// (broken) via a conditional group, with the param list an independent nested
// group so it breaks on its own width even when the body braces. Functions whose
// params/surrounds carry comments, whose params hold a brace-token default
// (`a = { ... }`, which the parser leaves as raw braces), or whose bare body is
// intrinsically multi-line fall back to the legacy string renderer (comment
// relocation and the `fits_with_newlines` wrap-in-place layout are unported).

pub(crate) fn ir_function_expr(
    node: &SyntaxNode,
    indent: usize,
    ctx: FormatContext,
) -> Result<Ir, FormatError> {
    let elements: Vec<_> = node.children_with_tokens().collect();
    let fn_idx = elements
        .iter()
        .position(
            |el| matches!(el, NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::FUNCTION_KW),
        )
        .ok_or_else(|| FormatError::AmbiguousConstruct {
            context: "missing 'function' keyword",
            snippet: node.text().to_string(),
        })?;
    let head = match &elements[fn_idx] {
        NodeOrToken::Token(tok) if tok.text() == "\\" => "\\",
        _ => "function",
    };
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

    let param_elements = &elements[lparen_idx + 1..rparen_idx];
    let body_elements = &elements[rparen_idx + 1..];

    // Comments are relocated natively: a comment before `(` is hoisted above the
    // whole definition; comments inside `()` keep the param list broken; a comment
    // between `)` and the body is lifted into (or braces) the body. With any such
    // comment the definition stays broken — the bare/flat inline form never applies.
    let leading_fn_comments: Vec<String> = elements[fn_idx + 1..lparen_idx]
        .iter()
        .filter_map(comment_text_of)
        .collect();
    let param_has_comment = param_elements
        .iter()
        .any(|el| el.kind() == SyntaxKind::COMMENT);

    let params_ir = if param_has_comment {
        ir_function_params_with_comments(param_elements)
    } else {
        ir_function_params(param_elements, indent, ctx)?
    };

    // Peel leading comments (between `)` and the body) off the body core.
    let mut body_leading_comments = Vec::new();
    let mut body_start = 0usize;
    while body_start < body_elements.len() {
        match &body_elements[body_start] {
            NodeOrToken::Token(tok) if super::super::core::is_trivia(tok.kind()) => body_start += 1,
            NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::COMMENT => {
                body_leading_comments.push(tok.text().to_string());
                body_start += 1;
            }
            _ => break,
        }
    }
    let body_core: Vec<_> = body_elements[body_start..]
        .iter()
        .filter(|el| !super::super::core::is_trivia(el.kind()))
        .cloned()
        .collect();
    if body_core.is_empty() {
        return Err(FormatError::AmbiguousConstruct {
            context: "missing function body expression",
            snippet: node.text().to_string(),
        });
    }

    let head_ir = Ir::text(head);
    let body_node = single_node(&body_core);

    let core = if let Some(block) = body_node
        .as_ref()
        .filter(|n| n.kind() == SyntaxKind::BLOCK_EXPR)
    {
        let block_ir =
            ir_block_expr_with_prefixed_comments(block, indent, ctx, &body_leading_comments)?;
        // A flattenable block (`function(p) { stmt }` → `function(p) stmt`) only
        // applies with no comments forcing the layout open.
        if !param_has_comment
            && body_leading_comments.is_empty()
            && let Some(stmt_ir) = try_flatten_function_block(block, indent, ctx)?
        {
            function_body_choice(head_ir, params_ir, stmt_ir, block_ir)
        } else {
            function_braced_hug(head_ir, params_ir, block_ir)
        }
    } else {
        // Bare (non-block) body. A body whose IR carries a forced break (e.g. an
        // `if` with braced arms) is kept bare and multi-line by the legacy
        // renderer when each line fits; defer to it (it relocates the comments too)
        // rather than force-bracing the body.
        let body_ir = ir_expr_segment(&body_core, "function body", indent, ctx)?;
        if body_ir.contains_forced_break() {
            return Ok(Ir::verbatim(format_function_expr(node, indent, ctx)?));
        }
        if !param_has_comment && body_leading_comments.is_empty() {
            function_body_choice(
                head_ir,
                params_ir,
                body_ir.clone(),
                brace_wrap_body(body_ir),
            )
        } else {
            function_braced_hug(
                head_ir,
                params_ir,
                brace_wrap_body_with_comments(body_ir, &body_leading_comments),
            )
        }
    };

    Ok(prepend_comment_lines(&leading_fn_comments, core))
}

/// Wrap a bare body in a block, prefixing `comments` on their own lines before
/// the body (`{`, `# c`, …, body, `}`). With no comments this is
/// [`brace_wrap_body`].
fn brace_wrap_body_with_comments(body: Ir, comments: &[String]) -> Ir {
    let mut inner: Vec<Ir> = Vec::new();
    for comment in comments {
        inner.push(Ir::hard_line());
        inner.push(Ir::verbatim_forced(comment.clone()));
    }
    inner.push(Ir::hard_line());
    inner.push(body);
    Ir::concat([
        Ir::text("{"),
        Ir::indent(Ir::concat(inner)),
        Ir::hard_line(),
        Ir::text("}"),
    ])
}

/// IR port of [`format_function_parameters`]'s comment branch: with a comment in
/// the param list, emit each comma-delimited segment's raw (trimmed) lines one
/// per line, a comma after the last line of every non-final segment. The list is
/// always broken.
fn ir_function_params_with_comments(param_elements: &[SyntaxElement<RLanguage>]) -> Ir {
    let segments = split_top_level_function_params(param_elements);
    if segments.is_empty() {
        return Ir::text("()");
    }
    let mut lines: Vec<Ir> = Vec::new();
    for (idx, segment) in segments.iter().enumerate() {
        let raw = snippet_from_elements(segment);
        let seg_lines: Vec<&str> = raw
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect();
        let last = seg_lines.len();
        for (line_idx, line) in seg_lines.iter().enumerate() {
            let add_comma = idx + 1 < segments.len() && line_idx + 1 == last;
            lines.push(Ir::text(if add_comma {
                format!("{line},")
            } else {
                (*line).to_string()
            }));
        }
    }
    Ir::concat([
        Ir::text("("),
        Ir::indent(Ir::concat([
            Ir::hard_line(),
            Ir::join(Ir::hard_line(), lines),
        ])),
        Ir::hard_line(),
        Ir::text(")"),
    ])
}

/// The conditional group choosing the bare inline body (flat) vs the braced
/// block (broken). The flat branch is measured by the outer group, so the bare
/// form is used exactly when `head(params) bare_body` fits. The broken branch is
/// the braced-block hug, which then decides the param-list break on its own.
fn function_body_choice(head: Ir, params: Ir, bare_body: Ir, braced_body: Ir) -> Ir {
    Ir::group(Ir::if_break(
        Ir::concat([head.clone(), params.clone(), Ir::text(" "), bare_body]),
        function_braced_hug(head, params, braced_body),
    ))
}

/// `head(params) <block>` as a hug group: the param list stays inline as long as
/// `head(params) {` fits (the printer's fit measurement stops at the block's
/// opening brace), otherwise the params break one per line. The block's own hard
/// breaks lay out its body regardless.
fn function_braced_hug(head: Ir, params: Ir, block: Ir) -> Ir {
    Ir::group_hug(Ir::concat([head, params, Ir::text(" "), block]))
}

/// Wrap a bare body expression in a block on its own indented line.
fn brace_wrap_body(body: Ir) -> Ir {
    Ir::concat([
        Ir::text("{"),
        Ir::indent(Ir::concat([Ir::hard_line(), body])),
        Ir::hard_line(),
        Ir::text("}"),
    ])
}

/// Build the `( ... )` param list as a bare concat (no enclosing group): empty
/// params collapse to `()`, otherwise the params are soft-line separated inside
/// an indent so an enclosing group can lay them inline or one per line. The
/// caller wraps this in the group that owns the break decision.
///
/// One exception forces the list to break: a brace-token default whose inner
/// expression is itself a literal block (`function(a = {{ var }})`). Legacy
/// triggers this on the formatted string (`param.contains("= {\n  {\n")`);
/// here we detect the same shape at the token level and wrap the params in
/// [`Ir::group_expanded`], pushing the brace default's `Verbatim` to be
/// rendered at `indent + 1` so the nested `{` lines up correctly.
fn ir_function_params(
    param_elements: &[SyntaxElement<RLanguage>],
    indent: usize,
    ctx: FormatContext,
) -> Result<Ir, FormatError> {
    let significant: Vec<_> = param_elements
        .iter()
        .filter(|el| !super::super::core::is_trivia(el.kind()))
        .cloned()
        .collect();
    if significant.is_empty() {
        return Ok(Ir::text("()"));
    }

    let segments = split_function_param_segments(&significant)?;
    let nested_brace = segments
        .iter()
        .any(|seg| param_has_nested_brace_default(seg));
    let param_indent = if nested_brace { indent + 1 } else { indent };
    let mut params: Vec<Ir> = Vec::with_capacity(segments.len());
    for param in &segments {
        params.push(ir_function_parameter(param, param_indent, ctx)?);
    }

    let mut body: Vec<Ir> = Vec::new();
    for (idx, param) in params.into_iter().enumerate() {
        if idx > 0 {
            body.push(Ir::if_break(Ir::text(", "), Ir::text(",")));
        }
        body.push(Ir::soft_line());
        body.push(param);
    }
    let inner = Ir::concat([
        Ir::text("("),
        Ir::indent(Ir::concat(body)),
        Ir::soft_line(),
        Ir::text(")"),
    ]);
    if nested_brace {
        Ok(Ir::group_expanded(inner))
    } else {
        Ok(inner)
    }
}

/// A param whose default is `{ <BLOCK> ... }` — a brace default whose inner
/// starts with another `{`. Matches the legacy
/// `param.contains("= {\n  {\n")` heuristic at the token level.
fn param_has_nested_brace_default(param: &[SyntaxElement<RLanguage>]) -> bool {
    let Some(eq_idx) = param
        .iter()
        .position(|el| matches!(el, NodeOrToken::Token(t) if t.kind() == SyntaxKind::ASSIGN_EQ))
    else {
        return false;
    };
    let default_significant: Vec<_> = param[eq_idx + 1..]
        .iter()
        .filter(|el| !super::super::core::is_trivia(el.kind()))
        .collect();
    if default_significant.len() < 4 {
        return false;
    }
    matches!(default_significant.first(), Some(NodeOrToken::Token(t)) if t.kind() == SyntaxKind::LBRACE)
        && matches!(default_significant.get(1), Some(NodeOrToken::Token(t)) if t.kind() == SyntaxKind::LBRACE)
        && matches!(default_significant.last(), Some(NodeOrToken::Token(t)) if t.kind() == SyntaxKind::RBRACE)
}

fn ir_function_parameter(
    param: &[SyntaxElement<RLanguage>],
    indent: usize,
    ctx: FormatContext,
) -> Result<Ir, FormatError> {
    if let Some(eq_idx) = param
        .iter()
        .position(|el| matches!(el, NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::ASSIGN_EQ))
    {
        let name = ir_expr_segment(&param[..eq_idx], "function parameter name", indent, ctx)?;
        let value = ir_function_param_default(&param[eq_idx + 1..], indent, ctx)?;
        return Ok(Ir::concat([name, Ir::text(" = "), value]));
    }
    ir_expr_segment(param, "function parameter", indent, ctx)
}

/// Render a parameter default. The parser builds no nodes inside the param list,
/// so a non-trivial default arrives as a raw run of tokens (`c(1, 2, 3)` is
/// `IDENT ( INT , INT , INT )`); reparse it into a single expression. Brace
/// defaults (`{ … }`) take a separate path: they are *always* multi-line and
/// are rendered relative to the function-expr's own indent (mirroring the
/// legacy `format_expr_or_braced_tokens`), independent of the enclosing
/// param-list `Ir::Indent`.
fn ir_function_param_default(
    elements: &[SyntaxElement<RLanguage>],
    indent: usize,
    ctx: FormatContext,
) -> Result<Ir, FormatError> {
    let significant: Vec<_> = elements
        .iter()
        .filter(|el| !super::super::core::is_trivia(el.kind()))
        .cloned()
        .collect();
    if let (Some(NodeOrToken::Token(lb)), Some(NodeOrToken::Token(rb))) =
        (significant.first(), significant.last())
        && lb.kind() == SyntaxKind::LBRACE
        && rb.kind() == SyntaxKind::RBRACE
    {
        return ir_brace_token_default(&significant, indent, ctx);
    }
    if let [only] = elements {
        return ir_expr_element(only, indent, ctx);
    }
    let snippet = snippet_from_elements(elements);
    let parsed = parse(&snippet);
    if !parsed.diagnostics.is_empty() {
        return Err(FormatError::AmbiguousConstruct {
            context: "function parameter default",
            snippet,
        });
    }
    let reparsed: Vec<_> = parsed
        .cst
        .children_with_tokens()
        .filter(|el| !super::super::core::is_trivia(el.kind()))
        .collect();
    let [only] = reparsed.as_slice() else {
        return Err(FormatError::AmbiguousConstruct {
            context: "function parameter default",
            snippet,
        });
    };
    ir_expr_element(only, indent, ctx)
}

/// Brace-token parameter default (`a = { … }`). The legacy renderer
/// (`format_expr_or_braced_tokens`) emits this as
/// `{\n<indent+1>inner\n<indent>}` using explicit indent strings tied to the
/// function-expr's own `indent`, not the param list's nested indent — so the
/// closing `}` lands at the function's level regardless of param-list
/// breaking. The IR's `Ir::Indent` doesn't have that affordance, so we
/// pre-render the inner expression and splice the multi-line braced form
/// through as a `Verbatim`, exactly matching the legacy layout.
fn ir_brace_token_default(
    significant: &[SyntaxElement<RLanguage>],
    indent: usize,
    ctx: FormatContext,
) -> Result<Ir, FormatError> {
    if significant.len() == 2 {
        return Ok(Ir::text("{}"));
    }
    let inner = &significant[1..significant.len() - 1];
    let snippet = snippet_from_elements(inner);
    let parsed = parse(&snippet);
    if !parsed.diagnostics.is_empty() {
        return Err(FormatError::AmbiguousConstruct {
            context: "function parameter brace default",
            snippet,
        });
    }
    let reparsed: Vec<_> = parsed
        .cst
        .children_with_tokens()
        .filter(|el| !super::super::core::is_trivia(el.kind()))
        .collect();
    let [only] = reparsed.as_slice() else {
        return Err(FormatError::AmbiguousConstruct {
            context: "function parameter brace default",
            snippet,
        });
    };
    let inner_ir = ir_expr_element(only, indent + 1, ctx)?;
    let inner_text =
        super::super::printer::Printer::new(ctx.style()).print_at(&inner_ir, indent + 1);
    Ok(Ir::verbatim_forced(format!(
        "{{\n{}{}\n{}}}",
        ctx.indent_text(indent + 1),
        inner_text,
        ctx.indent_text(indent)
    )))
}

/// Split params on top-level commas, preserving the legacy splitter's errors on
/// an empty parameter or a trailing comma.
fn split_function_param_segments(
    significant: &[SyntaxElement<RLanguage>],
) -> Result<Vec<Vec<SyntaxElement<RLanguage>>>, FormatError> {
    let mut params: Vec<Vec<SyntaxElement<RLanguage>>> = Vec::new();
    let mut current: Vec<SyntaxElement<RLanguage>> = Vec::new();
    let mut depth = 0usize;
    for element in significant {
        match element {
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
    if current.is_empty() {
        return Err(FormatError::AmbiguousConstruct {
            context: "trailing comma in function parameters",
            snippet: snippet_from_elements(significant),
        });
    }
    params.push(current);
    Ok(params)
}

/// When a block body is exactly one comment-free statement, return that
/// statement's IR so the printer can flatten `function(p) { stmt }` to
/// `function(p) stmt` when it fits; otherwise `None` (keep it braced).
fn try_flatten_function_block(
    block: &SyntaxNode,
    indent: usize,
    ctx: FormatContext,
) -> Result<Option<Ir>, FormatError> {
    if block
        .descendants_with_tokens()
        .any(|el| el.kind() == SyntaxKind::COMMENT)
    {
        return Ok(None);
    }
    let elements: Vec<_> = block.children_with_tokens().collect();
    let Some(open_idx) = elements
        .iter()
        .position(|el| matches!(el, NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::LBRACE))
    else {
        return Ok(None);
    };
    let Some(close_idx) = elements
        .iter()
        .rposition(|el| matches!(el, NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::RBRACE))
    else {
        return Ok(None);
    };
    if close_idx <= open_idx {
        return Ok(None);
    }
    let lines = split_lines(
        elements[open_idx + 1..close_idx].to_vec(),
        "function body block",
    )?;
    let mut stmt: Option<Ir> = None;
    for line in &lines {
        let ir = ir_line(line, indent, ctx)?;
        if matches!(ir, Ir::Nil) {
            continue;
        }
        if stmt.is_some() {
            return Ok(None);
        }
        stmt = Some(ir);
    }
    Ok(stmt)
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
    if out
        .iter()
        .any(|param| param.contains("= {\n  {\n") || param.contains("= {\n    {\n"))
    {
        let mut multiline = String::new();
        multiline.push('\n');
        for (idx, param) in out.iter().enumerate() {
            for line in param.lines() {
                multiline.push_str(&ctx.indent_text(indent + 1));
                multiline.push_str(line);
                multiline.push('\n');
            }
            if idx + 1 < out.len() {
                let insert_at = multiline.trim_end_matches('\n').len();
                multiline.insert(insert_at, ',');
            }
        }
        multiline.push_str(&ctx.indent_text(indent));
        return Ok(multiline);
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
    let inner_text = match format_expr_with_optional_comment(inner, context, indent + 1, ctx) {
        Ok(inner_text) => inner_text,
        Err(FormatError::AmbiguousConstruct { .. }) => {
            format_expr_tokens_via_parser(inner, context, indent + 1, ctx)?
        }
        Err(err) => return Err(err),
    };
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

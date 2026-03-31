use crate::parser::cursor::{skip_ws, skip_ws_and_newlines};
use crate::parser::diagnostics::{ParseDiagnostic, push_token_diagnostic};
use crate::parser::events::{Event, ExprParse};
use crate::parser::lexer::{TokKind, Token};
use crate::parser::recovery::error_expr_to_line_end;
use crate::parser::structural::{
    parse_for_expr, parse_function_expr, parse_if_expr, parse_repeat_expr, parse_while_expr,
};
use crate::syntax::SyntaxKind;

fn is_assignment_operator(kind: &TokKind) -> bool {
    matches!(
        kind,
        TokKind::AssignLeft
            | TokKind::SuperAssign
            | TokKind::AssignRight
            | TokKind::SuperAssignRight
            | TokKind::AssignEq
    )
}

fn is_infix_operator(kind: &TokKind) -> bool {
    infix_binding_power(kind).is_some()
}

fn next_operator(tokens: &[Token], start: usize) -> Option<(usize, &Token)> {
    let op_idx = skip_ws(tokens, start);
    let op = tokens.get(op_idx)?;
    if op.kind == TokKind::Newline {
        let next_idx = skip_ws_and_newlines(tokens, start);
        let next = tokens.get(next_idx)?;
        if is_assignment_operator(&next.kind) || is_infix_operator(&next.kind) {
            return Some((next_idx, next));
        }
        return None;
    }
    if is_assignment_operator(&op.kind) || is_infix_operator(&op.kind) {
        return Some((op_idx, op));
    }
    None
}

pub(crate) fn parse_expr(
    tokens: &[Token],
    start: usize,
    min_bp: u8,
    diagnostics: &mut Vec<ParseDiagnostic>,
) -> Option<ExprParse> {
    parse_expr_with_mode(tokens, start, min_bp, diagnostics, false)
}

fn parse_expr_with_mode(
    tokens: &[Token],
    start: usize,
    min_bp: u8,
    diagnostics: &mut Vec<ParseDiagnostic>,
    allow_newline_prefix: bool,
) -> Option<ExprParse> {
    let start_non_ws = if allow_newline_prefix {
        skip_ws_and_newlines(tokens, start)
    } else {
        skip_ws(tokens, start)
    };
    if matches!(
        tokens.get(start_non_ws).map(|t| &t.kind),
        Some(TokKind::IfKw)
    ) {
        return parse_if_expr(tokens, start_non_ws, diagnostics);
    }
    if matches!(
        tokens.get(start_non_ws).map(|t| &t.kind),
        Some(TokKind::ForKw)
    ) {
        return parse_for_expr(tokens, start_non_ws, diagnostics);
    }
    if matches!(
        tokens.get(start_non_ws).map(|t| &t.kind),
        Some(TokKind::WhileKw)
    ) {
        return parse_while_expr(tokens, start_non_ws, diagnostics);
    }
    if matches!(
        tokens.get(start_non_ws).map(|t| &t.kind),
        Some(TokKind::RepeatKw)
    ) {
        return parse_repeat_expr(tokens, start_non_ws, diagnostics);
    }
    if matches!(
        tokens.get(start_non_ws).map(|t| &t.kind),
        Some(TokKind::FunctionKw)
    ) {
        return parse_function_expr(tokens, start_non_ws, diagnostics);
    }

    let mut lhs = parse_prefix(tokens, start, diagnostics, allow_newline_prefix)?;

    loop {
        lhs = parse_postfix_chain(tokens, lhs, diagnostics);

        let Some((op_idx, op)) = next_operator(tokens, lhs.end) else {
            break;
        };

        if is_assignment_operator(&op.kind) {
            let (l_bp, r_bp) = (1, 1);
            if l_bp < min_bp {
                break;
            }

            let rhs_start = op_idx + 1;
            let rhs_allow_newline = op.kind == TokKind::Pipe || op.kind == TokKind::UserOp;
            let Some(rhs) =
                parse_expr_with_mode(tokens, rhs_start, r_bp, diagnostics, rhs_allow_newline)
            else {
                push_token_diagnostic(diagnostics, "expected assignment right-hand side", op);
                return Some(error_expr_to_line_end(tokens, lhs.start, rhs_start));
            };

            lhs = build_assignment_expr(lhs, op_idx, rhs);
            continue;
        }

        let Some((l_bp, r_bp)) = infix_binding_power(&op.kind) else {
            break;
        };
        if l_bp < min_bp {
            break;
        }

        let rhs_start = op_idx + 1;
        let rhs_allow_newline = op.kind == TokKind::Pipe || op.kind == TokKind::UserOp;
        let Some(rhs) =
            parse_expr_with_mode(tokens, rhs_start, r_bp, diagnostics, rhs_allow_newline)
        else {
            push_token_diagnostic(
                diagnostics,
                "expected right-hand side for binary operator",
                op,
            );
            return Some(error_expr_to_line_end(tokens, lhs.start, rhs_start));
        };

        lhs = build_binary_expr(lhs, op_idx, rhs);
    }

    Some(lhs)
}

fn parse_prefix(
    tokens: &[Token],
    start: usize,
    diagnostics: &mut Vec<ParseDiagnostic>,
    allow_newline_prefix: bool,
) -> Option<ExprParse> {
    let i = if allow_newline_prefix {
        skip_ws_and_newlines(tokens, start)
    } else {
        skip_ws(tokens, start)
    };
    let tok = tokens.get(i)?;

    match tok.kind {
        TokKind::Plus | TokKind::Minus | TokKind::Bang => {
            let operand_start = i + 1;
            let Some(operand) = parse_expr_with_mode(tokens, operand_start, 130, diagnostics, true)
            else {
                push_token_diagnostic(diagnostics, "expected operand for unary operator", tok);
                return Some(error_expr_to_line_end(tokens, i, operand_start));
            };
            let mut events = Vec::new();
            events.push(Event::Start(SyntaxKind::UNARY_EXPR));
            events.push(Event::Tok(i));
            for idx in (i + 1)..operand.start {
                events.push(Event::Tok(idx));
            }
            events.extend(operand.events);
            events.push(Event::Finish);
            Some(ExprParse {
                start: i,
                end: operand.end,
                events,
            })
        }
        TokKind::Ident
        | TokKind::Int
        | TokKind::Float
        | TokKind::String
        | TokKind::Comment
        | TokKind::UserOp => Some(ExprParse {
            start: i,
            end: i + 1,
            events: vec![Event::Tok(i)],
        }),
        TokKind::LParen => {
            let inner_start = i + 1;
            let mut expr_start = inner_start;
            while matches!(
                tokens.get(expr_start).map(|t| &t.kind),
                Some(TokKind::Whitespace | TokKind::Newline | TokKind::Comment)
            ) {
                expr_start += 1;
            }
            let Some(inner) = parse_expr_with_mode(tokens, expr_start, 0, diagnostics, true) else {
                push_token_diagnostic(diagnostics, "expected expression after '('", tok);
                return Some(error_expr_to_line_end(tokens, i, inner_start));
            };
            let close_idx = skip_ws_and_newlines(tokens, inner.end);
            if !matches!(
                tokens.get(close_idx).map(|t| &t.kind),
                Some(TokKind::RParen)
            ) {
                push_token_diagnostic(diagnostics, "expected ')'", tok);
                let mut events = Vec::new();
                events.push(Event::Start(SyntaxKind::PAREN_EXPR));
                events.push(Event::Tok(i));
                for idx in inner_start..inner.start {
                    events.push(Event::Tok(idx));
                }
                events.extend(inner.events);
                for idx in inner.end..close_idx {
                    events.push(Event::Tok(idx));
                }
                events.push(Event::Finish);
                return Some(ExprParse {
                    start: i,
                    end: close_idx,
                    events,
                });
            }

            let mut events = Vec::new();
            events.push(Event::Start(SyntaxKind::PAREN_EXPR));
            events.push(Event::Tok(i));
            for idx in inner_start..inner.start {
                events.push(Event::Tok(idx));
            }
            events.extend(inner.events);
            for idx in inner.end..close_idx {
                events.push(Event::Tok(idx));
            }
            events.push(Event::Tok(close_idx));
            events.push(Event::Finish);

            Some(ExprParse {
                start: i,
                end: close_idx + 1,
                events,
            })
        }
        TokKind::LBrace => parse_block_expr(tokens, i, diagnostics),
        TokKind::Star
        | TokKind::Slash
        | TokKind::Caret
        | TokKind::AssignLeft
        | TokKind::SuperAssign
        | TokKind::AssignRight
        | TokKind::SuperAssignRight
        | TokKind::AssignEq
        | TokKind::Colon
        | TokKind::Or
        | TokKind::Or2
        | TokKind::And
        | TokKind::And2
        | TokKind::Equal2
        | TokKind::NotEqual
        | TokKind::LessThan
        | TokKind::LessThanOrEqual
        | TokKind::GreaterThan
        | TokKind::GreaterThanOrEqual
        | TokKind::Tilde
        | TokKind::Colon2
        | TokKind::Colon3
        | TokKind::Dollar
        | TokKind::At
        | TokKind::Semicolon
        | TokKind::Comma
        | TokKind::Pipe => {
            push_token_diagnostic(diagnostics, "unexpected operator at expression start", tok);
            Some(error_expr_to_line_end(tokens, i, i + 1))
        }
        TokKind::Whitespace
        | TokKind::Newline
        | TokKind::RParen
        | TokKind::RBrack
        | TokKind::RBrack2
        | TokKind::RBrace
        | TokKind::IfKw
        | TokKind::ElseKw
        | TokKind::ForKw
        | TokKind::WhileKw
        | TokKind::RepeatKw
        | TokKind::FunctionKw
        | TokKind::InKw
        | TokKind::LBrack
        | TokKind::LBrack2
        | TokKind::Unknown => None,
    }
}

fn parse_block_expr(
    tokens: &[Token],
    start: usize,
    diagnostics: &mut Vec<ParseDiagnostic>,
) -> Option<ExprParse> {
    let mut i = start + 1;
    let mut events = vec![Event::Start(SyntaxKind::BLOCK_EXPR), Event::Tok(start)];

    loop {
        let next = skip_ws(tokens, i);
        let Some(tok) = tokens.get(next) else {
            push_token_diagnostic(diagnostics, "expected '}' to close block", &tokens[start]);
            for idx in i..tokens.len() {
                events.push(Event::Tok(idx));
            }
            events.push(Event::Finish);
            return Some(ExprParse {
                start,
                end: tokens.len(),
                events,
            });
        };

        if tok.kind == TokKind::RBrace {
            for idx in i..next {
                events.push(Event::Tok(idx));
            }
            events.push(Event::Tok(next));
            events.push(Event::Finish);
            return Some(ExprParse {
                start,
                end: next + 1,
                events,
            });
        }

        // Consume newlines and semicolons as statement separators.
        if matches!(tok.kind, TokKind::Newline | TokKind::Semicolon) {
            for idx in i..=next {
                events.push(Event::Tok(idx));
            }
            i = next + 1;
            continue;
        }

        if let Some(expr) = parse_expr(tokens, i, 0, diagnostics) {
            for idx in i..expr.start {
                events.push(Event::Tok(idx));
            }
            events.extend(expr.events);
            i = expr.end;
        } else {
            events.push(Event::Tok(i));
            i += 1;
        }
    }
}

fn parse_postfix_chain(
    tokens: &[Token],
    mut lhs: ExprParse,
    diagnostics: &mut Vec<ParseDiagnostic>,
) -> ExprParse {
    loop {
        // No newline is allowed between the callee and `(` — `f\n(x)` is two statements in R.
        let after_lhs = skip_ws(tokens, lhs.end);
        if matches!(
            tokens.get(after_lhs).map(|t| &t.kind),
            Some(TokKind::LParen)
        ) {
            lhs = parse_call_expr(tokens, lhs, after_lhs, diagnostics);
            continue;
        }
        if matches!(
            tokens.get(after_lhs).map(|t| &t.kind),
            Some(TokKind::LBrack)
        ) {
            lhs = parse_subset_expr(tokens, lhs, after_lhs, diagnostics);
            continue;
        }
        if matches!(
            tokens.get(after_lhs).map(|t| &t.kind),
            Some(TokKind::LBrack2)
        ) {
            lhs = parse_subset2_expr(tokens, lhs, after_lhs, diagnostics);
            continue;
        }
        break;
    }
    lhs
}

fn build_binary_like_expr(
    kind: SyntaxKind,
    lhs: ExprParse,
    op_idx: usize,
    rhs: ExprParse,
) -> ExprParse {
    let mut events = Vec::new();
    events.push(Event::Start(kind));
    events.extend(lhs.events);
    for idx in lhs.end..op_idx {
        events.push(Event::Tok(idx));
    }
    events.push(Event::Tok(op_idx));
    for idx in (op_idx + 1)..rhs.start {
        events.push(Event::Tok(idx));
    }
    events.extend(rhs.events);
    events.push(Event::Finish);

    ExprParse {
        start: lhs.start,
        end: rhs.end,
        events,
    }
}

fn build_binary_expr(lhs: ExprParse, op_idx: usize, rhs: ExprParse) -> ExprParse {
    build_binary_like_expr(SyntaxKind::BINARY_EXPR, lhs, op_idx, rhs)
}

fn build_assignment_expr(lhs: ExprParse, op_idx: usize, rhs: ExprParse) -> ExprParse {
    build_binary_like_expr(SyntaxKind::ASSIGNMENT_EXPR, lhs, op_idx, rhs)
}

/// Returns true if tokens starting at `i` match the pattern `ident =` (named argument),
/// where `=` is not `==`.
fn is_named_arg(tokens: &[Token], i: usize) -> bool {
    if !matches!(tokens.get(i).map(|t| &t.kind), Some(TokKind::Ident)) {
        return false;
    }
    let next = skip_ws(tokens, i + 1);
    matches!(tokens.get(next).map(|t| &t.kind), Some(TokKind::AssignEq))
}

fn has_newline(tokens: &[Token], start: usize, end: usize) -> bool {
    tokens[start..end]
        .iter()
        .any(|t| t.kind == TokKind::Newline)
}

fn parse_call_expr(
    tokens: &[Token],
    callee: ExprParse,
    lparen_idx: usize,
    diagnostics: &mut Vec<ParseDiagnostic>,
) -> ExprParse {
    let mut events = vec![Event::Start(SyntaxKind::CALL_EXPR)];
    events.extend(callee.events);
    // Whitespace between callee end and `(`
    for idx in callee.end..lparen_idx {
        events.push(Event::Tok(idx));
    }
    events.push(Event::Tok(lparen_idx)); // (

    events.push(Event::Start(SyntaxKind::ARG_LIST));
    let mut i = lparen_idx + 1;

    let mut expect_delimiter = false;
    loop {
        // Skip whitespace and newlines within the argument list.
        let next_i = skip_ws_and_newlines(tokens, i);
        let had_newline_gap = has_newline(tokens, i, next_i);
        for idx in i..next_i {
            events.push(Event::Tok(idx));
        }
        i = next_i;

        // End of argument list.
        if matches!(tokens.get(i).map(|t| &t.kind), Some(TokKind::RParen) | None) {
            break;
        }

        if expect_delimiter
            && !had_newline_gap
            && let Some(tok) = tokens.get(i)
        {
            push_token_diagnostic(diagnostics, "expected ',' between arguments", tok);
        }

        // Empty argument (leading or consecutive comma).
        if matches!(tokens.get(i).map(|t| &t.kind), Some(TokKind::Comma)) {
            events.push(Event::Start(SyntaxKind::ARG));
            events.push(Event::Finish);
            events.push(Event::Tok(i)); // ,
            i += 1;
            expect_delimiter = false;
            continue;
        }

        events.push(Event::Start(SyntaxKind::ARG));

        if is_named_arg(tokens, i) {
            // Named argument: ident = expr
            events.push(Event::Tok(i)); // ident
            let eq_idx = skip_ws(tokens, i + 1);
            for idx in (i + 1)..eq_idx {
                events.push(Event::Tok(idx));
            }
            events.push(Event::Tok(eq_idx)); // =
            let val_start = eq_idx + 1;
            if let Some(val) = parse_expr(tokens, val_start, 0, diagnostics) {
                for idx in val_start..val.start {
                    events.push(Event::Tok(idx));
                }
                events.extend(val.events);
                i = val.end;
            } else {
                i = val_start;
            }
        } else {
            // Positional argument.
            if let Some(arg) = parse_expr(tokens, i, 0, diagnostics) {
                for idx in i..arg.start {
                    events.push(Event::Tok(idx));
                }
                events.extend(arg.events);
                i = arg.end;
            } else {
                events.push(Event::Tok(i));
                i += 1;
            }
        }

        events.push(Event::Finish); // ARG
        expect_delimiter = true;

        // Skip whitespace/newlines after arg, then consume optional comma.
        let next_i = skip_ws_and_newlines(tokens, i);
        for idx in i..next_i {
            events.push(Event::Tok(idx));
        }
        i = next_i;

        if matches!(tokens.get(i).map(|t| &t.kind), Some(TokKind::Comma)) {
            events.push(Event::Tok(i)); // ,
            i += 1;
            expect_delimiter = false;
        }
    }

    events.push(Event::Finish); // ARG_LIST

    // Closing paren (may have trailing whitespace/newlines before it).
    let next_i = skip_ws_and_newlines(tokens, i);
    for idx in i..next_i {
        events.push(Event::Tok(idx));
    }
    i = next_i;

    if matches!(tokens.get(i).map(|t| &t.kind), Some(TokKind::RParen)) {
        events.push(Event::Tok(i));
        i += 1;
    } else if let Some(tok) = tokens.get(lparen_idx) {
        push_token_diagnostic(diagnostics, "expected ')' to close function call", tok);
    }

    events.push(Event::Finish); // CALL_EXPR

    ExprParse {
        start: callee.start,
        end: i,
        events,
    }
}

fn parse_subset_expr(
    tokens: &[Token],
    target: ExprParse,
    lbrack_idx: usize,
    diagnostics: &mut Vec<ParseDiagnostic>,
) -> ExprParse {
    parse_bracket_expr(
        tokens,
        target,
        lbrack_idx,
        TokKind::RBrack,
        SyntaxKind::SUBSET_EXPR,
        diagnostics,
    )
}

fn parse_subset2_expr(
    tokens: &[Token],
    target: ExprParse,
    lbrack2_idx: usize,
    diagnostics: &mut Vec<ParseDiagnostic>,
) -> ExprParse {
    parse_bracket_expr(
        tokens,
        target,
        lbrack2_idx,
        TokKind::RBrack2,
        SyntaxKind::SUBSET2_EXPR,
        diagnostics,
    )
}

fn parse_bracket_expr(
    tokens: &[Token],
    target: ExprParse,
    open_idx: usize,
    close_kind: TokKind,
    node_kind: SyntaxKind,
    diagnostics: &mut Vec<ParseDiagnostic>,
) -> ExprParse {
    let mut events = vec![Event::Start(node_kind)];
    events.extend(target.events);
    for idx in target.end..open_idx {
        events.push(Event::Tok(idx));
    }
    events.push(Event::Tok(open_idx));

    events.push(Event::Start(SyntaxKind::ARG_LIST));
    let mut i = open_idx + 1;

    let mut expect_delimiter = false;
    loop {
        let next_i = skip_ws_and_newlines(tokens, i);
        let had_newline_gap = has_newline(tokens, i, next_i);
        for idx in i..next_i {
            events.push(Event::Tok(idx));
        }
        i = next_i;

        if matches!(tokens.get(i).map(|t| &t.kind), Some(k) if *k == close_kind)
            || tokens.get(i).is_none()
        {
            break;
        }

        if expect_delimiter
            && !had_newline_gap
            && let Some(tok) = tokens.get(i)
        {
            push_token_diagnostic(diagnostics, "expected ',' between subset arguments", tok);
        }

        if matches!(tokens.get(i).map(|t| &t.kind), Some(TokKind::Comma)) {
            events.push(Event::Start(SyntaxKind::ARG));
            events.push(Event::Finish);
            events.push(Event::Tok(i));
            i += 1;
            expect_delimiter = false;
            continue;
        }

        events.push(Event::Start(SyntaxKind::ARG));
        if let Some(arg) = parse_expr(tokens, i, 0, diagnostics) {
            for idx in i..arg.start {
                events.push(Event::Tok(idx));
            }
            events.extend(arg.events);
            i = arg.end;
        } else {
            events.push(Event::Tok(i));
            i += 1;
        }
        events.push(Event::Finish);
        expect_delimiter = true;

        let next_i = skip_ws_and_newlines(tokens, i);
        for idx in i..next_i {
            events.push(Event::Tok(idx));
        }
        i = next_i;
        if matches!(tokens.get(i).map(|t| &t.kind), Some(TokKind::Comma)) {
            events.push(Event::Tok(i));
            i += 1;
            expect_delimiter = false;
        }
    }

    events.push(Event::Finish); // ARG_LIST
    let next_i = skip_ws_and_newlines(tokens, i);
    for idx in i..next_i {
        events.push(Event::Tok(idx));
    }
    i = next_i;

    if matches!(tokens.get(i).map(|t| &t.kind), Some(k) if *k == close_kind) {
        events.push(Event::Tok(i));
        i += 1;
    } else if let Some(tok) = tokens.get(open_idx) {
        push_token_diagnostic(
            diagnostics,
            "expected closing bracket in subset expression",
            tok,
        );
    }

    events.push(Event::Finish);
    ExprParse {
        start: target.start,
        end: i,
        events,
    }
}

fn infix_binding_power(kind: &TokKind) -> Option<(u8, u8)> {
    // Binding powers are aligned to AIR's operator precedence tiers:
    // LogicalOr (5), LogicalAnd (6), Relational (8), Additive (9),
    // Multiplicative (10), Special (11), Colon (12), Tilde (4), Exponential (14).
    // Namespace/extract operators (`::`, `:::`, `$`, `@`) bind tighter than
    // exponentiation and are treated as left-associative in this CST parser.
    match kind {
        TokKind::Or | TokKind::Or2 => Some((50, 51)),
        TokKind::And | TokKind::And2 => Some((60, 61)),
        TokKind::Equal2
        | TokKind::NotEqual
        | TokKind::LessThan
        | TokKind::LessThanOrEqual
        | TokKind::GreaterThan
        | TokKind::GreaterThanOrEqual => Some((80, 81)),
        TokKind::Plus | TokKind::Minus => Some((90, 91)),
        TokKind::Star | TokKind::Slash => Some((100, 101)),
        TokKind::Pipe | TokKind::UserOp => Some((110, 111)),
        TokKind::Colon => Some((120, 121)),
        TokKind::Tilde => Some((40, 41)),
        TokKind::Caret => Some((140, 140)),
        TokKind::Colon2 | TokKind::Colon3 | TokKind::Dollar | TokKind::At => Some((150, 151)),
        _ => None,
    }
}

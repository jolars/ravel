use crate::parser::cursor::skip_ws;
use crate::parser::diagnostics::{ParseDiagnostic, push_token_diagnostic};
use crate::parser::events::{Event, ExprParse};
use crate::parser::lexer::{TokKind, Token};
use crate::parser::recovery::error_expr_to_line_end;
use crate::parser::structural::{
    parse_for_expr, parse_function_expr, parse_if_expr, parse_while_expr,
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

pub(crate) fn parse_expr(
    tokens: &[Token],
    start: usize,
    min_bp: u8,
    diagnostics: &mut Vec<ParseDiagnostic>,
) -> Option<ExprParse> {
    let start_non_ws = skip_ws(tokens, start);
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
        Some(TokKind::FunctionKw)
    ) {
        return parse_function_expr(tokens, start_non_ws, diagnostics);
    }

    let mut lhs = parse_prefix(tokens, start, diagnostics)?;

    loop {
        let op_idx = skip_ws(tokens, lhs.end);
        let Some(op) = tokens.get(op_idx) else {
            break;
        };

        if is_assignment_operator(&op.kind) {
            let (l_bp, r_bp) = (1, 1);
            if l_bp < min_bp {
                break;
            }

            let rhs_start = op_idx + 1;
            let Some(rhs) = parse_expr(tokens, rhs_start, r_bp, diagnostics) else {
                push_token_diagnostic(diagnostics, "expected assignment right-hand side", op);
                return Some(error_expr_to_line_end(tokens, lhs.start, rhs_start));
            };

            let mut events = Vec::new();
            events.push(Event::Start(SyntaxKind::ASSIGNMENT_EXPR));
            for idx in lhs.start..op_idx {
                events.push(Event::Tok(idx));
            }
            events.push(Event::Tok(op_idx));
            for idx in (op_idx + 1)..rhs.start {
                events.push(Event::Tok(idx));
            }
            events.extend(rhs.events);
            events.push(Event::Finish);

            lhs = ExprParse {
                start: lhs.start,
                end: rhs.end,
                events,
            };
            continue;
        }

        let Some((l_bp, r_bp)) = infix_binding_power(&op.kind) else {
            break;
        };
        if l_bp < min_bp {
            break;
        }

        let rhs_start = op_idx + 1;
        let Some(rhs) = parse_expr(tokens, rhs_start, r_bp, diagnostics) else {
            push_token_diagnostic(
                diagnostics,
                "expected right-hand side for binary operator",
                op,
            );
            return Some(error_expr_to_line_end(tokens, lhs.start, rhs_start));
        };

        let mut events = Vec::new();
        events.push(Event::Start(SyntaxKind::BINARY_EXPR));
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

        lhs = ExprParse {
            start: lhs.start,
            end: rhs.end,
            events,
        };
    }

    Some(lhs)
}

fn parse_prefix(
    tokens: &[Token],
    start: usize,
    diagnostics: &mut Vec<ParseDiagnostic>,
) -> Option<ExprParse> {
    let i = skip_ws(tokens, start);
    let tok = tokens.get(i)?;

    match tok.kind {
        TokKind::Ident
        | TokKind::Int
        | TokKind::Float
        | TokKind::String
        | TokKind::Comment
        | TokKind::UserOp
        | TokKind::LBrack2
        | TokKind::RBrack2 => Some(ExprParse {
            start: i,
            end: i + 1,
            events: vec![Event::Tok(i)],
        }),
        TokKind::LParen => {
            let inner_start = i + 1;
            let Some(inner) = parse_expr(tokens, inner_start, 0, diagnostics) else {
                push_token_diagnostic(diagnostics, "expected expression after '('", tok);
                return Some(error_expr_to_line_end(tokens, i, inner_start));
            };
            let close_idx = skip_ws(tokens, inner.end);
            if !matches!(
                tokens.get(close_idx).map(|t| &t.kind),
                Some(TokKind::RParen)
            ) {
                push_token_diagnostic(diagnostics, "expected ')'", tok);
                let mut events = Vec::new();
                events.push(Event::Start(SyntaxKind::PAREN_EXPR));
                events.push(Event::Tok(i));
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
        TokKind::Plus
        | TokKind::Star
        | TokKind::Caret
        | TokKind::AssignLeft
        | TokKind::SuperAssign
        | TokKind::AssignRight
        | TokKind::SuperAssignRight
        | TokKind::AssignEq
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
        | TokKind::Pipe => {
            push_token_diagnostic(diagnostics, "unexpected operator at expression start", tok);
            Some(error_expr_to_line_end(tokens, i, i + 1))
        }
        TokKind::Whitespace
        | TokKind::Newline
        | TokKind::RParen
        | TokKind::RBrace
        | TokKind::IfKw
        | TokKind::ElseKw
        | TokKind::ForKw
        | TokKind::WhileKw
        | TokKind::FunctionKw
        | TokKind::InKw
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

fn infix_binding_power(kind: &TokKind) -> Option<(u8, u8)> {
    // Binding powers are aligned to AIR's operator precedence tiers:
    // LogicalOr (5), LogicalAnd (6), Relational (8), Additive (9),
    // Multiplicative (10), Special (11), Exponential (14).
    match kind {
        TokKind::Or | TokKind::Or2 => Some((50, 51)),
        TokKind::And | TokKind::And2 => Some((60, 61)),
        TokKind::Equal2
        | TokKind::NotEqual
        | TokKind::LessThan
        | TokKind::LessThanOrEqual
        | TokKind::GreaterThan
        | TokKind::GreaterThanOrEqual => Some((80, 81)),
        TokKind::Plus => Some((90, 91)),
        TokKind::Star => Some((100, 101)),
        TokKind::Pipe => Some((110, 111)),
        TokKind::Caret => Some((140, 140)),
        _ => None,
    }
}

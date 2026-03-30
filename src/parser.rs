use rowan::GreenNodeBuilder;

use crate::syntax::{SyntaxKind, SyntaxNode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseDiagnostic {
    pub message: String,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone)]
pub struct ParseOutput {
    pub cst: SyntaxNode,
    pub diagnostics: Vec<ParseDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TokKind {
    Ident,
    Int,
    Float,
    String,
    Comment,
    IfKw,
    ElseKw,
    ForKw,
    WhileKw,
    FunctionKw,
    InKw,
    UserOp,
    LBrack2,
    RBrack2,
    Plus,
    Star,
    Caret,
    LParen,
    RParen,
    LBrace,
    RBrace,
    AssignLeft,
    Whitespace,
    Newline,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Token {
    kind: TokKind,
    text: String,
    start: usize,
    end: usize,
}

#[derive(Debug, Clone)]
enum Event {
    Start(SyntaxKind),
    Tok(usize),
    Finish,
}

#[derive(Debug, Clone)]
struct ExprParse {
    start: usize,
    end: usize,
    events: Vec<Event>,
}

pub fn parse(text: &str) -> ParseOutput {
    let tokens = lex(text);
    let mut diagnostics = Vec::new();
    let mut root_events = Vec::new();

    let mut i = 0usize;
    while i < tokens.len() {
        if matches!(tokens[i].kind, TokKind::Whitespace | TokKind::Newline) {
            root_events.push(Event::Tok(i));
            i += 1;
            continue;
        }

        if let Some(expr) = parse_expr(&tokens, i, 0, &mut diagnostics) {
            root_events.extend(expr.events);
            i = expr.end;
        } else {
            root_events.push(Event::Tok(i));
            i += 1;
        }
    }

    let cst = build_tree(&tokens, &root_events);
    ParseOutput { cst, diagnostics }
}

pub fn reconstruct(text: &str) -> String {
    parse(text)
        .cst
        .descendants_with_tokens()
        .filter_map(|el| el.into_token())
        .map(|tok| tok.text().to_string())
        .collect::<String>()
}

fn parse_expr(
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

        if op.kind == TokKind::AssignLeft {
            let (l_bp, r_bp) = (1, 1);
            if l_bp < min_bp {
                break;
            }

            let rhs_start = op_idx + 1;
            let Some(rhs) = parse_expr(tokens, rhs_start, r_bp, diagnostics) else {
                diagnostics.push(ParseDiagnostic {
                    message: "expected assignment right-hand side".to_string(),
                    start: op.start,
                    end: op.end,
                });
                let end_idx = consume_to_line_end(tokens, rhs_start);
                let mut events = Vec::new();
                events.push(Event::Start(SyntaxKind::ERROR));
                for idx in lhs.start..end_idx {
                    events.push(Event::Tok(idx));
                }
                events.push(Event::Finish);
                return Some(ExprParse {
                    start: lhs.start,
                    end: end_idx,
                    events,
                });
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
            diagnostics.push(ParseDiagnostic {
                message: "expected right-hand side for binary operator".to_string(),
                start: op.start,
                end: op.end,
            });
            let end_idx = consume_to_line_end(tokens, rhs_start);
            let mut events = Vec::new();
            events.push(Event::Start(SyntaxKind::ERROR));
            for idx in lhs.start..end_idx {
                events.push(Event::Tok(idx));
            }
            events.push(Event::Finish);
            return Some(ExprParse {
                start: lhs.start,
                end: end_idx,
                events,
            });
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

fn parse_if_expr(
    tokens: &[Token],
    start: usize,
    diagnostics: &mut Vec<ParseDiagnostic>,
) -> Option<ExprParse> {
    let if_tok = tokens.get(start)?;
    let mut events = vec![Event::Start(SyntaxKind::IF_EXPR), Event::Tok(start)];
    let mut cursor = start + 1;
    let mut cond_start = skip_ws_and_newlines(tokens, cursor);
    let mut saw_lparen = false;

    if matches!(
        tokens.get(cond_start).map(|t| &t.kind),
        Some(TokKind::LParen)
    ) {
        push_range(&mut events, cursor, cond_start);
        events.push(Event::Tok(cond_start));
        cursor = cond_start + 1;
        cond_start = cursor;
        saw_lparen = true;
    } else {
        diagnostics.push(ParseDiagnostic {
            message: "expected '(' after 'if'".to_string(),
            start: if_tok.start,
            end: if_tok.end,
        });
        push_range(&mut events, cursor, cond_start);
        cursor = cond_start;
    }

    if let Some(cond) = parse_expr(tokens, cond_start, 0, diagnostics) {
        push_range(&mut events, cursor, cond.start);
        events.extend(cond.events);
        cursor = cond.end;
    } else {
        diagnostics.push(ParseDiagnostic {
            message: "expected condition expression after 'if'".to_string(),
            start: if_tok.start,
            end: if_tok.end,
        });
        events.push(Event::Start(SyntaxKind::ERROR));
        events.push(Event::Finish);
        cursor = cond_start;
    }

    if saw_lparen {
        let cond_rparen = skip_ws_and_newlines(tokens, cursor);
        if matches!(
            tokens.get(cond_rparen).map(|t| &t.kind),
            Some(TokKind::RParen)
        ) {
            push_range(&mut events, cursor, cond_rparen);
            events.push(Event::Tok(cond_rparen));
            cursor = cond_rparen + 1;
        } else {
            diagnostics.push(ParseDiagnostic {
                message: "expected ')' after if condition".to_string(),
                start: if_tok.start,
                end: if_tok.end,
            });
        }
    }

    let then_start = skip_ws_and_newlines(tokens, cursor);
    if let Some(then_expr) = parse_expr(tokens, then_start, 0, diagnostics) {
        push_range(&mut events, cursor, then_expr.start);
        events.extend(then_expr.events);
        cursor = then_expr.end;
    } else {
        diagnostics.push(ParseDiagnostic {
            message: "expected expression after if condition".to_string(),
            start: if_tok.start,
            end: if_tok.end,
        });
        let recovery = skip_ws_and_newlines(tokens, cursor);
        push_range(&mut events, cursor, recovery);
        events.push(Event::Start(SyntaxKind::ERROR));
        events.push(Event::Finish);
        cursor = recovery;
    }

    let else_idx = skip_ws(tokens, cursor);
    if matches!(tokens.get(else_idx).map(|t| &t.kind), Some(TokKind::ElseKw)) {
        push_range(&mut events, cursor, else_idx);
        events.push(Event::Tok(else_idx));
        cursor = else_idx + 1;

        if let Some(parsed_else) = parse_expr(tokens, cursor, 0, diagnostics) {
            push_range(&mut events, cursor, parsed_else.start);
            events.extend(parsed_else.events);
            cursor = parsed_else.end;
        } else {
            diagnostics.push(ParseDiagnostic {
                message: "expected expression after 'else'".to_string(),
                start: tokens[else_idx].start,
                end: tokens[else_idx].end,
            });
            let recovery = skip_ws(tokens, cursor);
            push_range(&mut events, cursor, recovery);
            events.push(Event::Start(SyntaxKind::ERROR));
            events.push(Event::Finish);
            cursor = recovery;
        }
    }

    events.push(Event::Finish);
    Some(ExprParse {
        start,
        end: cursor,
        events,
    })
}

fn parse_while_expr(
    tokens: &[Token],
    start: usize,
    diagnostics: &mut Vec<ParseDiagnostic>,
) -> Option<ExprParse> {
    let while_tok = tokens.get(start)?;
    let mut events = vec![Event::Start(SyntaxKind::WHILE_EXPR), Event::Tok(start)];
    let mut cursor = start + 1;
    let mut cond_start = skip_ws_and_newlines(tokens, cursor);
    let mut saw_lparen = false;

    if matches!(
        tokens.get(cond_start).map(|t| &t.kind),
        Some(TokKind::LParen)
    ) {
        push_range(&mut events, cursor, cond_start);
        events.push(Event::Tok(cond_start));
        cursor = cond_start + 1;
        cond_start = cursor;
        saw_lparen = true;
    } else {
        diagnostics.push(ParseDiagnostic {
            message: "expected '(' after 'while'".to_string(),
            start: while_tok.start,
            end: while_tok.end,
        });
        push_range(&mut events, cursor, cond_start);
        cursor = cond_start;
    }

    if let Some(cond) = parse_expr(tokens, cond_start, 0, diagnostics) {
        push_range(&mut events, cursor, cond.start);
        events.extend(cond.events);
        cursor = cond.end;
    } else {
        diagnostics.push(ParseDiagnostic {
            message: "expected condition expression after 'while'".to_string(),
            start: while_tok.start,
            end: while_tok.end,
        });
        events.push(Event::Start(SyntaxKind::ERROR));
        events.push(Event::Finish);
        cursor = cond_start;
    }

    if saw_lparen {
        let cond_rparen = skip_ws_and_newlines(tokens, cursor);
        if matches!(
            tokens.get(cond_rparen).map(|t| &t.kind),
            Some(TokKind::RParen)
        ) {
            push_range(&mut events, cursor, cond_rparen);
            events.push(Event::Tok(cond_rparen));
            cursor = cond_rparen + 1;
        } else {
            diagnostics.push(ParseDiagnostic {
                message: "expected ')' after while condition".to_string(),
                start: while_tok.start,
                end: while_tok.end,
            });
            events.push(Event::Start(SyntaxKind::ERROR));
            events.push(Event::Finish);
        }
    }

    let body_start = skip_ws_and_newlines(tokens, cursor);
    if let Some(body_expr) = parse_expr(tokens, body_start, 0, diagnostics) {
        push_range(&mut events, cursor, body_expr.start);
        events.extend(body_expr.events);
        cursor = body_expr.end;
    } else {
        diagnostics.push(ParseDiagnostic {
            message: "expected expression after while condition".to_string(),
            start: while_tok.start,
            end: while_tok.end,
        });
        let recovery = skip_ws_and_newlines(tokens, cursor);
        push_range(&mut events, cursor, recovery);
        events.push(Event::Start(SyntaxKind::ERROR));
        events.push(Event::Finish);
        cursor = recovery;
    }

    events.push(Event::Finish);
    Some(ExprParse {
        start,
        end: cursor,
        events,
    })
}

fn parse_for_expr(
    tokens: &[Token],
    start: usize,
    diagnostics: &mut Vec<ParseDiagnostic>,
) -> Option<ExprParse> {
    let for_tok = tokens.get(start)?;
    let mut events = vec![Event::Start(SyntaxKind::FOR_EXPR), Event::Tok(start)];
    let mut cursor = start + 1;
    let clause_start = skip_ws_and_newlines(tokens, cursor);
    let mut saw_lparen = false;

    if matches!(
        tokens.get(clause_start).map(|t| &t.kind),
        Some(TokKind::LParen)
    ) {
        push_range(&mut events, cursor, clause_start);
        events.push(Event::Tok(clause_start));
        cursor = clause_start + 1;
        saw_lparen = true;
    } else {
        diagnostics.push(ParseDiagnostic {
            message: "expected '(' after 'for'".to_string(),
            start: for_tok.start,
            end: for_tok.end,
        });
        push_range(&mut events, cursor, clause_start);
        cursor = clause_start;
    }

    let var_start = skip_ws_and_newlines(tokens, cursor);
    if matches!(tokens.get(var_start).map(|t| &t.kind), Some(TokKind::Ident)) {
        push_range(&mut events, cursor, var_start);
        events.push(Event::Tok(var_start));
        cursor = var_start + 1;
    } else {
        diagnostics.push(ParseDiagnostic {
            message: "expected loop variable after '(' in 'for'".to_string(),
            start: for_tok.start,
            end: for_tok.end,
        });
        push_range(&mut events, cursor, var_start);
        events.push(Event::Start(SyntaxKind::ERROR));
        events.push(Event::Finish);
        cursor = var_start;
    }

    let in_idx = skip_ws_and_newlines(tokens, cursor);
    if matches!(tokens.get(in_idx).map(|t| &t.kind), Some(TokKind::InKw)) {
        push_range(&mut events, cursor, in_idx);
        events.push(Event::Tok(in_idx));
        cursor = in_idx + 1;
    } else {
        diagnostics.push(ParseDiagnostic {
            message: "expected 'in' after for variable".to_string(),
            start: for_tok.start,
            end: for_tok.end,
        });
        push_range(&mut events, cursor, in_idx);
        events.push(Event::Start(SyntaxKind::ERROR));
        events.push(Event::Finish);
        cursor = in_idx;
    }

    let seq_start = skip_ws_and_newlines(tokens, cursor);
    if let Some(seq_expr) = parse_expr(tokens, seq_start, 0, diagnostics) {
        push_range(&mut events, cursor, seq_expr.start);
        events.extend(seq_expr.events);
        cursor = seq_expr.end;
    } else {
        diagnostics.push(ParseDiagnostic {
            message: "expected sequence expression after 'in'".to_string(),
            start: for_tok.start,
            end: for_tok.end,
        });
        push_range(&mut events, cursor, seq_start);
        events.push(Event::Start(SyntaxKind::ERROR));
        events.push(Event::Finish);
        cursor = seq_start;
    }

    if saw_lparen {
        let clause_rparen = skip_ws_and_newlines(tokens, cursor);
        if matches!(
            tokens.get(clause_rparen).map(|t| &t.kind),
            Some(TokKind::RParen)
        ) {
            push_range(&mut events, cursor, clause_rparen);
            events.push(Event::Tok(clause_rparen));
            cursor = clause_rparen + 1;
        } else {
            diagnostics.push(ParseDiagnostic {
                message: "expected ')' after for clause".to_string(),
                start: for_tok.start,
                end: for_tok.end,
            });
            events.push(Event::Start(SyntaxKind::ERROR));
            events.push(Event::Finish);
        }
    }

    let body_start = skip_ws_and_newlines(tokens, cursor);
    if let Some(body_expr) = parse_expr(tokens, body_start, 0, diagnostics) {
        push_range(&mut events, cursor, body_expr.start);
        events.extend(body_expr.events);
        cursor = body_expr.end;
    } else {
        diagnostics.push(ParseDiagnostic {
            message: "expected expression after for clause".to_string(),
            start: for_tok.start,
            end: for_tok.end,
        });
        let recovery = skip_ws_and_newlines(tokens, cursor);
        push_range(&mut events, cursor, recovery);
        events.push(Event::Start(SyntaxKind::ERROR));
        events.push(Event::Finish);
        cursor = recovery;
    }

    events.push(Event::Finish);
    Some(ExprParse {
        start,
        end: cursor,
        events,
    })
}

fn parse_function_expr(
    tokens: &[Token],
    start: usize,
    diagnostics: &mut Vec<ParseDiagnostic>,
) -> Option<ExprParse> {
    let function_tok = tokens.get(start)?;
    let mut events = vec![Event::Start(SyntaxKind::FUNCTION_EXPR), Event::Tok(start)];
    let mut cursor = start + 1;
    let params_lparen = skip_ws_and_newlines(tokens, cursor);

    if matches!(
        tokens.get(params_lparen).map(|t| &t.kind),
        Some(TokKind::LParen)
    ) {
        push_range(&mut events, cursor, params_lparen);
        events.push(Event::Tok(params_lparen));
        cursor = params_lparen + 1;

        let mut i = cursor;
        let mut depth = 1usize;
        while i < tokens.len() {
            match tokens[i].kind {
                TokKind::LParen => depth += 1,
                TokKind::RParen => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
            i += 1;
        }

        if i < tokens.len() && matches!(tokens[i].kind, TokKind::RParen) {
            push_range(&mut events, cursor, i);
            events.push(Event::Tok(i));
            cursor = i + 1;
        } else {
            diagnostics.push(ParseDiagnostic {
                message: "expected ')' after function parameters".to_string(),
                start: function_tok.start,
                end: function_tok.end,
            });
            let recovery = find_function_body_recovery(tokens, cursor);
            push_range(&mut events, cursor, recovery);
            events.push(Event::Start(SyntaxKind::ERROR));
            events.push(Event::Finish);
            cursor = recovery;
        }
    } else {
        diagnostics.push(ParseDiagnostic {
            message: "expected '(' after 'function'".to_string(),
            start: function_tok.start,
            end: function_tok.end,
        });
        push_range(&mut events, cursor, params_lparen);
        cursor = params_lparen;
    }

    let body_start = skip_ws_and_newlines(tokens, cursor);
    if let Some(body_expr) = parse_expr(tokens, body_start, 0, diagnostics) {
        push_range(&mut events, cursor, body_expr.start);
        events.extend(body_expr.events);
        cursor = body_expr.end;
    } else {
        diagnostics.push(ParseDiagnostic {
            message: "expected expression after function parameters".to_string(),
            start: function_tok.start,
            end: function_tok.end,
        });
        let recovery = skip_ws_and_newlines(tokens, cursor);
        push_range(&mut events, cursor, recovery);
        events.push(Event::Start(SyntaxKind::ERROR));
        events.push(Event::Finish);
        cursor = recovery;
    }

    events.push(Event::Finish);
    Some(ExprParse {
        start,
        end: cursor,
        events,
    })
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
                diagnostics.push(ParseDiagnostic {
                    message: "expected expression after '('".to_string(),
                    start: tok.start,
                    end: tok.end,
                });
                let end_idx = consume_to_line_end(tokens, inner_start);
                let mut events = Vec::new();
                events.push(Event::Start(SyntaxKind::ERROR));
                for idx in i..end_idx {
                    events.push(Event::Tok(idx));
                }
                events.push(Event::Finish);
                return Some(ExprParse {
                    start: i,
                    end: end_idx,
                    events,
                });
            };
            let close_idx = skip_ws(tokens, inner.end);
            if !matches!(
                tokens.get(close_idx).map(|t| &t.kind),
                Some(TokKind::RParen)
            ) {
                diagnostics.push(ParseDiagnostic {
                    message: "expected ')'".to_string(),
                    start: tok.start,
                    end: tok.end,
                });
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
        TokKind::Plus | TokKind::Star | TokKind::Caret | TokKind::AssignLeft => {
            diagnostics.push(ParseDiagnostic {
                message: "unexpected operator at expression start".to_string(),
                start: tok.start,
                end: tok.end,
            });
            let end_idx = consume_to_line_end(tokens, i + 1);
            let mut events = Vec::new();
            events.push(Event::Start(SyntaxKind::ERROR));
            for idx in i..end_idx {
                events.push(Event::Tok(idx));
            }
            events.push(Event::Finish);
            Some(ExprParse {
                start: i,
                end: end_idx,
                events,
            })
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
            diagnostics.push(ParseDiagnostic {
                message: "expected '}' to close block".to_string(),
                start: tokens[start].start,
                end: tokens[start].end,
            });
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

fn push_range(events: &mut Vec<Event>, start: usize, end: usize) {
    for idx in start..end {
        events.push(Event::Tok(idx));
    }
}

fn build_tree(tokens: &[Token], events: &[Event]) -> SyntaxNode {
    let mut builder = GreenNodeBuilder::new();
    builder.start_node(SyntaxKind::ROOT.into());

    for event in events {
        match *event {
            Event::Start(kind) => builder.start_node(kind.into()),
            Event::Tok(idx) => push_token(&mut builder, &tokens[idx]),
            Event::Finish => builder.finish_node(),
        }
    }

    builder.finish_node();
    let green = builder.finish();
    SyntaxNode::new_root(green)
}

fn consume_to_line_end(tokens: &[Token], mut i: usize) -> usize {
    while i < tokens.len() && !matches!(tokens[i].kind, TokKind::Newline) {
        i += 1;
    }
    if i < tokens.len() && matches!(tokens[i].kind, TokKind::Newline) {
        i += 1;
    }
    i
}

fn find_function_body_recovery(tokens: &[Token], start: usize) -> usize {
    for (i, tok) in tokens.iter().enumerate().skip(start) {
        if matches!(tok.kind, TokKind::Newline) {
            return i;
        }
    }
    tokens.len()
}

fn infix_binding_power(kind: &TokKind) -> Option<(u8, u8)> {
    match kind {
        TokKind::Plus => Some((10, 11)),
        TokKind::Star => Some((20, 21)),
        TokKind::Caret => Some((30, 30)),
        _ => None,
    }
}

fn skip_ws(tokens: &[Token], mut i: usize) -> usize {
    while matches!(tokens.get(i).map(|t| &t.kind), Some(TokKind::Whitespace)) {
        i += 1;
    }
    i
}

fn skip_ws_and_newlines(tokens: &[Token], mut i: usize) -> usize {
    while matches!(
        tokens.get(i).map(|t| &t.kind),
        Some(TokKind::Whitespace | TokKind::Newline)
    ) {
        i += 1;
    }
    i
}

fn push_token(builder: &mut GreenNodeBuilder<'_>, tok: &Token) {
    let sk = match tok.kind {
        TokKind::Ident => SyntaxKind::IDENT,
        TokKind::Int => SyntaxKind::INT,
        TokKind::Float => SyntaxKind::FLOAT,
        TokKind::String => SyntaxKind::STRING,
        TokKind::Comment => SyntaxKind::COMMENT,
        TokKind::UserOp => SyntaxKind::USER_OP,
        TokKind::LBrack2 => SyntaxKind::LBRACK2,
        TokKind::RBrack2 => SyntaxKind::RBRACK2,
        TokKind::Plus => SyntaxKind::PLUS,
        TokKind::Star => SyntaxKind::STAR,
        TokKind::Caret => SyntaxKind::CARET,
        TokKind::LParen => SyntaxKind::LPAREN,
        TokKind::RParen => SyntaxKind::RPAREN,
        TokKind::IfKw => SyntaxKind::IF_KW,
        TokKind::ElseKw => SyntaxKind::ELSE_KW,
        TokKind::ForKw => SyntaxKind::FOR_KW,
        TokKind::WhileKw => SyntaxKind::WHILE_KW,
        TokKind::FunctionKw => SyntaxKind::FUNCTION_KW,
        TokKind::InKw => SyntaxKind::IN_KW,
        TokKind::LBrace => SyntaxKind::LBRACE,
        TokKind::RBrace => SyntaxKind::RBRACE,
        TokKind::AssignLeft => SyntaxKind::ASSIGN_LEFT,
        TokKind::Whitespace => SyntaxKind::WHITESPACE,
        TokKind::Newline => SyntaxKind::NEWLINE,
        TokKind::Unknown => SyntaxKind::ERROR,
    };
    builder.token(sk.into(), tok.text.as_str());
}

fn lex(input: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let mut i = 0usize;
    let bytes = input.as_bytes();

    while i < bytes.len() {
        let c = bytes[i] as char;

        if c == '\n' {
            out.push(Token {
                kind: TokKind::Newline,
                text: "\n".to_string(),
                start: i,
                end: i + 1,
            });
            i += 1;
            continue;
        }

        if c == '#' {
            let start = i;
            i += 1;
            while i < bytes.len() && (bytes[i] as char) != '\n' {
                i += 1;
            }
            out.push(Token {
                kind: TokKind::Comment,
                text: input[start..i].to_string(),
                start,
                end: i,
            });
            continue;
        }

        if c.is_ascii_whitespace() {
            let start = i;
            while i < bytes.len() {
                let ch = bytes[i] as char;
                if ch == '\n' || !ch.is_ascii_whitespace() {
                    break;
                }
                i += 1;
            }
            out.push(Token {
                kind: TokKind::Whitespace,
                text: input[start..i].to_string(),
                start,
                end: i,
            });
            continue;
        }

        if i + 1 < bytes.len() && &input[i..i + 2] == "<-" {
            out.push(Token {
                kind: TokKind::AssignLeft,
                text: "<-".to_string(),
                start: i,
                end: i + 2,
            });
            i += 2;
            continue;
        }

        if i + 1 < bytes.len() && &input[i..i + 2] == "[[" {
            out.push(Token {
                kind: TokKind::LBrack2,
                text: "[[".to_string(),
                start: i,
                end: i + 2,
            });
            i += 2;
            continue;
        }

        if i + 1 < bytes.len() && &input[i..i + 2] == "]]" {
            out.push(Token {
                kind: TokKind::RBrack2,
                text: "]]".to_string(),
                start: i,
                end: i + 2,
            });
            i += 2;
            continue;
        }

        if c == '+' {
            out.push(Token {
                kind: TokKind::Plus,
                text: "+".to_string(),
                start: i,
                end: i + 1,
            });
            i += 1;
            continue;
        }

        if c == '*' {
            out.push(Token {
                kind: TokKind::Star,
                text: "*".to_string(),
                start: i,
                end: i + 1,
            });
            i += 1;
            continue;
        }

        if c == '^' {
            out.push(Token {
                kind: TokKind::Caret,
                text: "^".to_string(),
                start: i,
                end: i + 1,
            });
            i += 1;
            continue;
        }

        if c == '(' {
            out.push(Token {
                kind: TokKind::LParen,
                text: "(".to_string(),
                start: i,
                end: i + 1,
            });
            i += 1;
            continue;
        }

        if c == ')' {
            out.push(Token {
                kind: TokKind::RParen,
                text: ")".to_string(),
                start: i,
                end: i + 1,
            });
            i += 1;
            continue;
        }

        if c == '{' {
            out.push(Token {
                kind: TokKind::LBrace,
                text: "{".to_string(),
                start: i,
                end: i + 1,
            });
            i += 1;
            continue;
        }

        if c == '}' {
            out.push(Token {
                kind: TokKind::RBrace,
                text: "}".to_string(),
                start: i,
                end: i + 1,
            });
            i += 1;
            continue;
        }

        if c == '%' {
            let start = i;
            i += 1;
            while i < bytes.len() && (bytes[i] as char) != '%' {
                i += 1;
            }
            if i < bytes.len() && (bytes[i] as char) == '%' {
                i += 1;
                out.push(Token {
                    kind: TokKind::UserOp,
                    text: input[start..i].to_string(),
                    start,
                    end: i,
                });
            } else {
                out.push(Token {
                    kind: TokKind::Unknown,
                    text: input[start..i].to_string(),
                    start,
                    end: i,
                });
            }
            continue;
        }

        if c == '"' || c == '\'' {
            let quote = c;
            let start = i;
            i += 1;
            while i < bytes.len() {
                let ch = bytes[i] as char;
                if ch == '\\' && i + 1 < bytes.len() {
                    i += 2;
                    continue;
                }
                i += 1;
                if ch == quote {
                    break;
                }
            }
            out.push(Token {
                kind: TokKind::String,
                text: input[start..i].to_string(),
                start,
                end: i,
            });
            continue;
        }

        if c.is_ascii_digit() {
            let start = i;
            i += 1;
            while i < bytes.len() && (bytes[i] as char).is_ascii_digit() {
                i += 1;
            }
            if i < bytes.len()
                && (bytes[i] as char) == '.'
                && i + 1 < bytes.len()
                && (bytes[i + 1] as char).is_ascii_digit()
            {
                i += 1;
                while i < bytes.len() && (bytes[i] as char).is_ascii_digit() {
                    i += 1;
                }
                out.push(Token {
                    kind: TokKind::Float,
                    text: input[start..i].to_string(),
                    start,
                    end: i,
                });
            } else {
                out.push(Token {
                    kind: TokKind::Int,
                    text: input[start..i].to_string(),
                    start,
                    end: i,
                });
            }
            continue;
        }

        if c.is_ascii_alphabetic() || c == '_' {
            let start = i;
            i += 1;
            while i < bytes.len() {
                let ch = bytes[i] as char;
                if !(ch.is_ascii_alphanumeric() || ch == '_') {
                    break;
                }
                i += 1;
            }
            let text = &input[start..i];
            let kind = match text {
                "if" => TokKind::IfKw,
                "else" => TokKind::ElseKw,
                "for" => TokKind::ForKw,
                "while" => TokKind::WhileKw,
                "function" => TokKind::FunctionKw,
                "in" => TokKind::InKw,
                _ => TokKind::Ident,
            };
            out.push(Token {
                kind,
                text: text.to_string(),
                start,
                end: i,
            });
            continue;
        }

        out.push(Token {
            kind: TokKind::Unknown,
            text: c.to_string(),
            start: i,
            end: i + 1,
        });
        i += 1;
    }

    out
}

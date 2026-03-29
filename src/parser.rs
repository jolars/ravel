use rowan::GreenNodeBuilder;

use crate::syntax::{SyntaxKind, SyntaxNode};

#[derive(Debug, Clone, PartialEq, Eq)]
enum TokKind {
    Ident,
    Int,
    AssignLeft,
    Whitespace,
    Newline,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Token {
    kind: TokKind,
    text: String,
}

pub fn parse_text(text: &str) -> SyntaxNode {
    let tokens = lex(text);
    let mut builder = GreenNodeBuilder::new();
    builder.start_node(SyntaxKind::ROOT.into());

    let mut i = 0usize;
    while i < tokens.len() {
        if let Some((next, ok)) = parse_assignment_expr(&tokens, i, &mut builder) {
            i = next;
            if !ok {
                break;
            }
            continue;
        }

        push_token(&mut builder, &tokens[i]);
        i += 1;
    }

    builder.finish_node();
    let green = builder.finish();
    SyntaxNode::new_root(green)
}

pub fn debug_tree(text: &str) -> String {
    let node = parse_text(text);
    format!("{node:#?}")
}

pub fn reconstruct(text: &str) -> String {
    parse_text(text)
        .descendants_with_tokens()
        .filter_map(|el| el.into_token())
        .map(|tok| tok.text().to_string())
        .collect::<String>()
}

fn parse_assignment_expr(
    tokens: &[Token],
    start: usize,
    builder: &mut GreenNodeBuilder<'_>,
) -> Option<(usize, bool)> {
    let mut i = skip_ws(tokens, start);
    if !matches!(tokens.get(i).map(|t| &t.kind), Some(TokKind::Ident)) {
        return None;
    }

    let ident_idx = i;
    i += 1;
    i = skip_ws(tokens, i);

    if !matches!(tokens.get(i).map(|t| &t.kind), Some(TokKind::AssignLeft)) {
        return None;
    }

    let assign_idx = i;
    i += 1;
    i = skip_ws(tokens, i);

    if !matches!(tokens.get(i).map(|t| &t.kind), Some(TokKind::Int)) {
        builder.start_node(SyntaxKind::ERROR.into());
        for tok in &tokens[start..i] {
            push_token(builder, tok);
        }
        builder.finish_node();
        return Some((i, false));
    }

    let int_idx = i;
    i += 1;

    while matches!(
        tokens.get(i).map(|t| &t.kind),
        Some(TokKind::Whitespace | TokKind::Newline)
    ) {
        i += 1;
    }

    builder.start_node(SyntaxKind::ASSIGNMENT_EXPR.into());
    for tok in &tokens[start..ident_idx] {
        push_token(builder, tok);
    }
    push_token(builder, &tokens[ident_idx]);
    for tok in &tokens[ident_idx + 1..assign_idx] {
        push_token(builder, tok);
    }
    push_token(builder, &tokens[assign_idx]);
    for tok in &tokens[assign_idx + 1..int_idx] {
        push_token(builder, tok);
    }
    push_token(builder, &tokens[int_idx]);
    for tok in &tokens[int_idx + 1..i] {
        push_token(builder, tok);
    }
    builder.finish_node();

    Some((i, true))
}

fn skip_ws(tokens: &[Token], mut i: usize) -> usize {
    while matches!(tokens.get(i).map(|t| &t.kind), Some(TokKind::Whitespace)) {
        i += 1;
    }
    i
}

fn push_token(builder: &mut GreenNodeBuilder<'_>, tok: &Token) {
    let sk = match tok.kind {
        TokKind::Ident => SyntaxKind::IDENT,
        TokKind::Int => SyntaxKind::INT,
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
            });
            i += 1;
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
            });
            continue;
        }

        if i + 1 < bytes.len() && &input[i..i + 2] == "<-" {
            out.push(Token {
                kind: TokKind::AssignLeft,
                text: "<-".to_string(),
            });
            i += 2;
            continue;
        }

        if c.is_ascii_digit() {
            let start = i;
            i += 1;
            while i < bytes.len() && (bytes[i] as char).is_ascii_digit() {
                i += 1;
            }
            out.push(Token {
                kind: TokKind::Int,
                text: input[start..i].to_string(),
            });
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
            out.push(Token {
                kind: TokKind::Ident,
                text: input[start..i].to_string(),
            });
            continue;
        }

        out.push(Token {
            kind: TokKind::Unknown,
            text: c.to_string(),
        });
        i += 1;
    }

    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TokKind {
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
pub(crate) struct Token {
    pub(crate) kind: TokKind,
    pub(crate) text: String,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

pub(crate) fn lex(input: &str) -> Vec<Token> {
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

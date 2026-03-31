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
    Tilde,
    UserOp,
    LBrack2,
    RBrack2,
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    Pipe,
    Colon,
    Colon2,
    Colon3,
    Dollar,
    At,
    Semicolon,
    Comma,
    Or,
    Or2,
    And,
    And2,
    Equal2,
    NotEqual,
    Bang,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
    LParen,
    RParen,
    LBrace,
    RBrace,
    AssignLeft,
    SuperAssign,
    AssignRight,
    SuperAssignRight,
    AssignEq,
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

        match c {
            '\r' => {
                if i + 1 < bytes.len() && (bytes[i + 1] as char) == '\n' {
                    out.push(Token {
                        kind: TokKind::Newline,
                        text: "\r\n".to_string(),
                        start: i,
                        end: i + 2,
                    });
                    i += 2;
                } else {
                    out.push(Token {
                        kind: TokKind::Newline,
                        text: "\r".to_string(),
                        start: i,
                        end: i + 1,
                    });
                    i += 1;
                }
            }
            '\n' => {
                out.push(Token {
                    kind: TokKind::Newline,
                    text: "\n".to_string(),
                    start: i,
                    end: i + 1,
                });
                i += 1;
            }
            '#' => {
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
            }
            '~' => {
                out.push(Token {
                    kind: TokKind::Tilde,
                    text: "~".to_string(),
                    start: i,
                    end: i + 1,
                });
                i += 1;
            }
            '$' => {
                out.push(Token {
                    kind: TokKind::Dollar,
                    text: "$".to_string(),
                    start: i,
                    end: i + 1,
                });
                i += 1;
            }
            '@' => {
                out.push(Token {
                    kind: TokKind::At,
                    text: "@".to_string(),
                    start: i,
                    end: i + 1,
                });
                i += 1;
            }
            ';' => {
                out.push(Token {
                    kind: TokKind::Semicolon,
                    text: ";".to_string(),
                    start: i,
                    end: i + 1,
                });
                i += 1;
            }
            ',' => {
                out.push(Token {
                    kind: TokKind::Comma,
                    text: ",".to_string(),
                    start: i,
                    end: i + 1,
                });
                i += 1;
            }
            '+' => {
                out.push(Token {
                    kind: TokKind::Plus,
                    text: "+".to_string(),
                    start: i,
                    end: i + 1,
                });
                i += 1;
            }
            '*' => {
                out.push(Token {
                    kind: TokKind::Star,
                    text: "*".to_string(),
                    start: i,
                    end: i + 1,
                });
                i += 1;
            }
            '^' => {
                out.push(Token {
                    kind: TokKind::Caret,
                    text: "^".to_string(),
                    start: i,
                    end: i + 1,
                });
                i += 1;
            }
            '(' => {
                out.push(Token {
                    kind: TokKind::LParen,
                    text: "(".to_string(),
                    start: i,
                    end: i + 1,
                });
                i += 1;
            }
            ')' => {
                out.push(Token {
                    kind: TokKind::RParen,
                    text: ")".to_string(),
                    start: i,
                    end: i + 1,
                });
                i += 1;
            }
            '{' => {
                out.push(Token {
                    kind: TokKind::LBrace,
                    text: "{".to_string(),
                    start: i,
                    end: i + 1,
                });
                i += 1;
            }
            '}' => {
                out.push(Token {
                    kind: TokKind::RBrace,
                    text: "}".to_string(),
                    start: i,
                    end: i + 1,
                });
                i += 1;
            }
            '%' => {
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
            }
            '"' | '\'' => {
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
            }
            _ => {
                if c.is_ascii_whitespace() {
                    let start = i;
                    while i < bytes.len() {
                        let ch = bytes[i] as char;
                        if ch == '\n' || ch == '\r' || !ch.is_ascii_whitespace() {
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

                if i + 1 < bytes.len() && &input[i..i + 2] == "||" {
                    out.push(Token {
                        kind: TokKind::Or2,
                        text: "||".to_string(),
                        start: i,
                        end: i + 2,
                    });
                    i += 2;
                    continue;
                }

                if i + 1 < bytes.len() && &input[i..i + 2] == "&&" {
                    out.push(Token {
                        kind: TokKind::And2,
                        text: "&&".to_string(),
                        start: i,
                        end: i + 2,
                    });
                    i += 2;
                    continue;
                }

                if i + 1 < bytes.len() && &input[i..i + 2] == "==" {
                    out.push(Token {
                        kind: TokKind::Equal2,
                        text: "==".to_string(),
                        start: i,
                        end: i + 2,
                    });
                    i += 2;
                    continue;
                }

                if i + 1 < bytes.len() && &input[i..i + 2] == "!=" {
                    out.push(Token {
                        kind: TokKind::NotEqual,
                        text: "!=".to_string(),
                        start: i,
                        end: i + 2,
                    });
                    i += 2;
                    continue;
                }

                if c == '!' {
                    out.push(Token {
                        kind: TokKind::Bang,
                        text: "!".to_string(),
                        start: i,
                        end: i + 1,
                    });
                    i += 1;
                    continue;
                }

                if i + 2 < bytes.len() && &input[i..i + 3] == ":::" {
                    out.push(Token {
                        kind: TokKind::Colon3,
                        text: ":::".to_string(),
                        start: i,
                        end: i + 3,
                    });
                    i += 3;
                    continue;
                }

                if i + 1 < bytes.len() && &input[i..i + 2] == "::" {
                    out.push(Token {
                        kind: TokKind::Colon2,
                        text: "::".to_string(),
                        start: i,
                        end: i + 2,
                    });
                    i += 2;
                    continue;
                }

                if i + 1 < bytes.len() && &input[i..i + 2] == "|>" {
                    out.push(Token {
                        kind: TokKind::Pipe,
                        text: "|>".to_string(),
                        start: i,
                        end: i + 2,
                    });
                    i += 2;
                    continue;
                }

                if i + 2 < bytes.len() && &input[i..i + 3] == "<<-" {
                    out.push(Token {
                        kind: TokKind::SuperAssign,
                        text: "<<-".to_string(),
                        start: i,
                        end: i + 3,
                    });
                    i += 3;
                    continue;
                }

                if i + 2 < bytes.len() && &input[i..i + 3] == "->>" {
                    out.push(Token {
                        kind: TokKind::SuperAssignRight,
                        text: "->>".to_string(),
                        start: i,
                        end: i + 3,
                    });
                    i += 3;
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

                if i + 1 < bytes.len() && &input[i..i + 2] == "->" {
                    out.push(Token {
                        kind: TokKind::AssignRight,
                        text: "->".to_string(),
                        start: i,
                        end: i + 2,
                    });
                    i += 2;
                    continue;
                }

                if c == '-' {
                    out.push(Token {
                        kind: TokKind::Minus,
                        text: "-".to_string(),
                        start: i,
                        end: i + 1,
                    });
                    i += 1;
                    continue;
                }

                if c == '/' {
                    out.push(Token {
                        kind: TokKind::Slash,
                        text: "/".to_string(),
                        start: i,
                        end: i + 1,
                    });
                    i += 1;
                    continue;
                }

                if c == ':' {
                    out.push(Token {
                        kind: TokKind::Colon,
                        text: ":".to_string(),
                        start: i,
                        end: i + 1,
                    });
                    i += 1;
                    continue;
                }

                if i + 1 < bytes.len() && &input[i..i + 2] == "<=" {
                    out.push(Token {
                        kind: TokKind::LessThanOrEqual,
                        text: "<=".to_string(),
                        start: i,
                        end: i + 2,
                    });
                    i += 2;
                    continue;
                }

                if i + 1 < bytes.len() && &input[i..i + 2] == ">=" {
                    out.push(Token {
                        kind: TokKind::GreaterThanOrEqual,
                        text: ">=".to_string(),
                        start: i,
                        end: i + 2,
                    });
                    i += 2;
                    continue;
                }

                if c == '=' {
                    out.push(Token {
                        kind: TokKind::AssignEq,
                        text: "=".to_string(),
                        start: i,
                        end: i + 1,
                    });
                    i += 1;
                    continue;
                }

                if c == '|' {
                    out.push(Token {
                        kind: TokKind::Or,
                        text: "|".to_string(),
                        start: i,
                        end: i + 1,
                    });
                    i += 1;
                    continue;
                }

                if c == '&' {
                    out.push(Token {
                        kind: TokKind::And,
                        text: "&".to_string(),
                        start: i,
                        end: i + 1,
                    });
                    i += 1;
                    continue;
                }

                if c == '<' {
                    out.push(Token {
                        kind: TokKind::LessThan,
                        text: "<".to_string(),
                        start: i,
                        end: i + 1,
                    });
                    i += 1;
                    continue;
                }

                if c == '>' {
                    out.push(Token {
                        kind: TokKind::GreaterThan,
                        text: ">".to_string(),
                        start: i,
                        end: i + 1,
                    });
                    i += 1;
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
                        if !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '.') {
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
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::{TokKind, lex};

    #[test]
    fn lexes_crlf_as_single_newline_token() {
        let tokens = lex("x\r\ny");
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0].kind, TokKind::Ident);
        assert_eq!(tokens[1].kind, TokKind::Newline);
        assert_eq!(tokens[1].text, "\r\n");
        assert_eq!(tokens[2].kind, TokKind::Ident);
    }

    #[test]
    fn lexes_lone_cr_as_newline_token() {
        let tokens = lex("x\ry");
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0].kind, TokKind::Ident);
        assert_eq!(tokens[1].kind, TokKind::Newline);
        assert_eq!(tokens[1].text, "\r");
        assert_eq!(tokens[2].kind, TokKind::Ident);
    }

    #[test]
    fn lexes_dotted_identifier_as_single_ident_token() {
        let tokens = lex("is.null(x)");
        assert_eq!(tokens[0].kind, TokKind::Ident);
        assert_eq!(tokens[0].text, "is.null");
    }
}

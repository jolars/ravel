use rowan::Language;

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
pub enum SyntaxKind {
    ROOT,
    BINARY_EXPR,
    ASSIGNMENT_EXPR,
    PAREN_EXPR,
    IDENT,
    INT,
    FLOAT,
    STRING,
    COMMENT,
    USER_OP,
    LBRACK2,
    RBRACK2,
    PLUS,
    STAR,
    CARET,
    LPAREN,
    RPAREN,
    WHITESPACE,
    NEWLINE,
    ASSIGN_LEFT,
    ERROR,
}

impl From<SyntaxKind> for rowan::SyntaxKind {
    fn from(kind: SyntaxKind) -> Self {
        Self(kind as u16)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RLanguage {}

impl Language for RLanguage {
    type Kind = SyntaxKind;

    fn kind_from_raw(raw: rowan::SyntaxKind) -> Self::Kind {
        match raw.0 {
            0 => SyntaxKind::ROOT,
            1 => SyntaxKind::BINARY_EXPR,
            2 => SyntaxKind::ASSIGNMENT_EXPR,
            3 => SyntaxKind::PAREN_EXPR,
            4 => SyntaxKind::IDENT,
            5 => SyntaxKind::INT,
            6 => SyntaxKind::FLOAT,
            7 => SyntaxKind::STRING,
            8 => SyntaxKind::COMMENT,
            9 => SyntaxKind::USER_OP,
            10 => SyntaxKind::LBRACK2,
            11 => SyntaxKind::RBRACK2,
            12 => SyntaxKind::PLUS,
            13 => SyntaxKind::STAR,
            14 => SyntaxKind::CARET,
            15 => SyntaxKind::LPAREN,
            16 => SyntaxKind::RPAREN,
            17 => SyntaxKind::WHITESPACE,
            18 => SyntaxKind::NEWLINE,
            19 => SyntaxKind::ASSIGN_LEFT,
            20 => SyntaxKind::ERROR,
            _ => SyntaxKind::ERROR,
        }
    }

    fn kind_to_raw(kind: Self::Kind) -> rowan::SyntaxKind {
        kind.into()
    }
}

pub type SyntaxNode = rowan::SyntaxNode<RLanguage>;

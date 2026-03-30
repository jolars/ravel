use rowan::Language;

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
pub enum SyntaxKind {
    ROOT,
    BINARY_EXPR,
    ASSIGNMENT_EXPR,
    PAREN_EXPR,
    IF_EXPR,
    BLOCK_EXPR,
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
    IF_KW,
    ELSE_KW,
    LBRACE,
    RBRACE,
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
            4 => SyntaxKind::IF_EXPR,
            5 => SyntaxKind::BLOCK_EXPR,
            6 => SyntaxKind::IDENT,
            7 => SyntaxKind::INT,
            8 => SyntaxKind::FLOAT,
            9 => SyntaxKind::STRING,
            10 => SyntaxKind::COMMENT,
            11 => SyntaxKind::USER_OP,
            12 => SyntaxKind::LBRACK2,
            13 => SyntaxKind::RBRACK2,
            14 => SyntaxKind::PLUS,
            15 => SyntaxKind::STAR,
            16 => SyntaxKind::CARET,
            17 => SyntaxKind::LPAREN,
            18 => SyntaxKind::RPAREN,
            19 => SyntaxKind::IF_KW,
            20 => SyntaxKind::ELSE_KW,
            21 => SyntaxKind::LBRACE,
            22 => SyntaxKind::RBRACE,
            23 => SyntaxKind::WHITESPACE,
            24 => SyntaxKind::NEWLINE,
            25 => SyntaxKind::ASSIGN_LEFT,
            26 => SyntaxKind::ERROR,
            _ => SyntaxKind::ERROR,
        }
    }

    fn kind_to_raw(kind: Self::Kind) -> rowan::SyntaxKind {
        kind.into()
    }
}

pub type SyntaxNode = rowan::SyntaxNode<RLanguage>;

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
    FOR_EXPR,
    WHILE_EXPR,
    FUNCTION_EXPR,
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
    FOR_KW,
    WHILE_KW,
    FUNCTION_KW,
    IN_KW,
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
            5 => SyntaxKind::FOR_EXPR,
            6 => SyntaxKind::WHILE_EXPR,
            7 => SyntaxKind::FUNCTION_EXPR,
            8 => SyntaxKind::BLOCK_EXPR,
            9 => SyntaxKind::IDENT,
            10 => SyntaxKind::INT,
            11 => SyntaxKind::FLOAT,
            12 => SyntaxKind::STRING,
            13 => SyntaxKind::COMMENT,
            14 => SyntaxKind::USER_OP,
            15 => SyntaxKind::LBRACK2,
            16 => SyntaxKind::RBRACK2,
            17 => SyntaxKind::PLUS,
            18 => SyntaxKind::STAR,
            19 => SyntaxKind::CARET,
            20 => SyntaxKind::LPAREN,
            21 => SyntaxKind::RPAREN,
            22 => SyntaxKind::IF_KW,
            23 => SyntaxKind::ELSE_KW,
            24 => SyntaxKind::FOR_KW,
            25 => SyntaxKind::WHILE_KW,
            26 => SyntaxKind::FUNCTION_KW,
            27 => SyntaxKind::IN_KW,
            28 => SyntaxKind::LBRACE,
            29 => SyntaxKind::RBRACE,
            30 => SyntaxKind::WHITESPACE,
            31 => SyntaxKind::NEWLINE,
            32 => SyntaxKind::ASSIGN_LEFT,
            33 => SyntaxKind::ERROR,
            _ => SyntaxKind::ERROR,
        }
    }

    fn kind_to_raw(kind: Self::Kind) -> rowan::SyntaxKind {
        kind.into()
    }
}

pub type SyntaxNode = rowan::SyntaxNode<RLanguage>;

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
    PIPE,
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
    SUPER_ASSIGN,
    ASSIGN_RIGHT,
    SUPER_ASSIGN_RIGHT,
    ASSIGN_EQ,
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
            20 => SyntaxKind::PIPE,
            21 => SyntaxKind::LPAREN,
            22 => SyntaxKind::RPAREN,
            23 => SyntaxKind::IF_KW,
            24 => SyntaxKind::ELSE_KW,
            25 => SyntaxKind::FOR_KW,
            26 => SyntaxKind::WHILE_KW,
            27 => SyntaxKind::FUNCTION_KW,
            28 => SyntaxKind::IN_KW,
            29 => SyntaxKind::LBRACE,
            30 => SyntaxKind::RBRACE,
            31 => SyntaxKind::WHITESPACE,
            32 => SyntaxKind::NEWLINE,
            33 => SyntaxKind::ASSIGN_LEFT,
            34 => SyntaxKind::SUPER_ASSIGN,
            35 => SyntaxKind::ASSIGN_RIGHT,
            36 => SyntaxKind::SUPER_ASSIGN_RIGHT,
            37 => SyntaxKind::ASSIGN_EQ,
            38 => SyntaxKind::ERROR,
            _ => SyntaxKind::ERROR,
        }
    }

    fn kind_to_raw(kind: Self::Kind) -> rowan::SyntaxKind {
        kind.into()
    }
}

pub type SyntaxNode = rowan::SyntaxNode<RLanguage>;

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
    OR,
    OR2,
    AND,
    AND2,
    EQUAL2,
    NOT_EQUAL,
    LESS_THAN,
    LESS_THAN_OR_EQUAL,
    GREATER_THAN,
    GREATER_THAN_OR_EQUAL,
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
            21 => SyntaxKind::OR,
            22 => SyntaxKind::OR2,
            23 => SyntaxKind::AND,
            24 => SyntaxKind::AND2,
            25 => SyntaxKind::EQUAL2,
            26 => SyntaxKind::NOT_EQUAL,
            27 => SyntaxKind::LESS_THAN,
            28 => SyntaxKind::LESS_THAN_OR_EQUAL,
            29 => SyntaxKind::GREATER_THAN,
            30 => SyntaxKind::GREATER_THAN_OR_EQUAL,
            31 => SyntaxKind::LPAREN,
            32 => SyntaxKind::RPAREN,
            33 => SyntaxKind::IF_KW,
            34 => SyntaxKind::ELSE_KW,
            35 => SyntaxKind::FOR_KW,
            36 => SyntaxKind::WHILE_KW,
            37 => SyntaxKind::FUNCTION_KW,
            38 => SyntaxKind::IN_KW,
            39 => SyntaxKind::LBRACE,
            40 => SyntaxKind::RBRACE,
            41 => SyntaxKind::WHITESPACE,
            42 => SyntaxKind::NEWLINE,
            43 => SyntaxKind::ASSIGN_LEFT,
            44 => SyntaxKind::SUPER_ASSIGN,
            45 => SyntaxKind::ASSIGN_RIGHT,
            46 => SyntaxKind::SUPER_ASSIGN_RIGHT,
            47 => SyntaxKind::ASSIGN_EQ,
            48 => SyntaxKind::ERROR,
            _ => SyntaxKind::ERROR,
        }
    }

    fn kind_to_raw(kind: Self::Kind) -> rowan::SyntaxKind {
        kind.into()
    }
}

pub type SyntaxNode = rowan::SyntaxNode<RLanguage>;

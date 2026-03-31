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
    TILDE,
    USER_OP,
    LBRACK2,
    RBRACK2,
    PLUS,
    MINUS,
    STAR,
    SLASH,
    CARET,
    PIPE,
    COLON,
    COLON2,
    COLON3,
    DOLLAR,
    AT,
    SEMICOLON,
    COMMA,
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
            14 => SyntaxKind::TILDE,
            15 => SyntaxKind::USER_OP,
            16 => SyntaxKind::LBRACK2,
            17 => SyntaxKind::RBRACK2,
            18 => SyntaxKind::PLUS,
            19 => SyntaxKind::MINUS,
            20 => SyntaxKind::STAR,
            21 => SyntaxKind::SLASH,
            22 => SyntaxKind::CARET,
            23 => SyntaxKind::PIPE,
            24 => SyntaxKind::COLON,
            25 => SyntaxKind::COLON2,
            26 => SyntaxKind::COLON3,
            27 => SyntaxKind::DOLLAR,
            28 => SyntaxKind::AT,
            29 => SyntaxKind::SEMICOLON,
            30 => SyntaxKind::COMMA,
            31 => SyntaxKind::OR,
            32 => SyntaxKind::OR2,
            33 => SyntaxKind::AND,
            34 => SyntaxKind::AND2,
            35 => SyntaxKind::EQUAL2,
            36 => SyntaxKind::NOT_EQUAL,
            37 => SyntaxKind::LESS_THAN,
            38 => SyntaxKind::LESS_THAN_OR_EQUAL,
            39 => SyntaxKind::GREATER_THAN,
            40 => SyntaxKind::GREATER_THAN_OR_EQUAL,
            41 => SyntaxKind::LPAREN,
            42 => SyntaxKind::RPAREN,
            43 => SyntaxKind::IF_KW,
            44 => SyntaxKind::ELSE_KW,
            45 => SyntaxKind::FOR_KW,
            46 => SyntaxKind::WHILE_KW,
            47 => SyntaxKind::FUNCTION_KW,
            48 => SyntaxKind::IN_KW,
            49 => SyntaxKind::LBRACE,
            50 => SyntaxKind::RBRACE,
            51 => SyntaxKind::WHITESPACE,
            52 => SyntaxKind::NEWLINE,
            53 => SyntaxKind::ASSIGN_LEFT,
            54 => SyntaxKind::SUPER_ASSIGN,
            55 => SyntaxKind::ASSIGN_RIGHT,
            56 => SyntaxKind::SUPER_ASSIGN_RIGHT,
            57 => SyntaxKind::ASSIGN_EQ,
            58 => SyntaxKind::ERROR,
            _ => SyntaxKind::ERROR,
        }
    }

    fn kind_to_raw(kind: Self::Kind) -> rowan::SyntaxKind {
        kind.into()
    }
}

pub type SyntaxNode = rowan::SyntaxNode<RLanguage>;

use rowan::Language;

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
pub enum SyntaxKind {
    ROOT,
    ASSIGNMENT_EXPR,
    IDENT,
    INT,
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
            1 => SyntaxKind::ASSIGNMENT_EXPR,
            2 => SyntaxKind::IDENT,
            3 => SyntaxKind::INT,
            4 => SyntaxKind::WHITESPACE,
            5 => SyntaxKind::NEWLINE,
            6 => SyntaxKind::ASSIGN_LEFT,
            7 => SyntaxKind::ERROR,
            _ => SyntaxKind::ERROR,
        }
    }

    fn kind_to_raw(kind: Self::Kind) -> rowan::SyntaxKind {
        kind.into()
    }
}

pub type SyntaxNode = rowan::SyntaxNode<RLanguage>;

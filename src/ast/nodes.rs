use rowan::ast::support;

use crate::ast::AstNode;
use crate::syntax::{RLanguage, SyntaxKind, SyntaxNode};

macro_rules! ast_node {
    ($name:ident, $kind:expr) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct $name(SyntaxNode);

        impl AstNode for $name {
            type Language = RLanguage;

            fn can_cast(kind: SyntaxKind) -> bool {
                kind == $kind
            }

            fn cast(syntax: SyntaxNode) -> Option<Self> {
                Self::can_cast(syntax.kind()).then(|| Self(syntax))
            }

            fn syntax(&self) -> &SyntaxNode {
                &self.0
            }
        }
    };
}

ast_node!(Root, SyntaxKind::ROOT);
ast_node!(AssignmentExpr, SyntaxKind::ASSIGNMENT_EXPR);
ast_node!(BinaryExpr, SyntaxKind::BINARY_EXPR);
ast_node!(UnaryExpr, SyntaxKind::UNARY_EXPR);
ast_node!(ParenExpr, SyntaxKind::PAREN_EXPR);
ast_node!(CallExpr, SyntaxKind::CALL_EXPR);
ast_node!(ArgList, SyntaxKind::ARG_LIST);
ast_node!(Arg, SyntaxKind::ARG);
ast_node!(IfExpr, SyntaxKind::IF_EXPR);
ast_node!(ForExpr, SyntaxKind::FOR_EXPR);
ast_node!(FunctionExpr, SyntaxKind::FUNCTION_EXPR);
ast_node!(BlockExpr, SyntaxKind::BLOCK_EXPR);

impl Root {
    pub fn expressions(&self) -> impl Iterator<Item = SyntaxNode> {
        self.syntax().children()
    }
}

impl CallExpr {
    pub fn arg_list(&self) -> Option<ArgList> {
        support::child(self.syntax())
    }
}

impl ArgList {
    pub fn args(&self) -> impl Iterator<Item = Arg> {
        support::children(self.syntax())
    }
}

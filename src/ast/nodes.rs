use rowan::ast::support;
use rowan::{SyntaxElement, SyntaxToken};

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
ast_node!(WhileExpr, SyntaxKind::WHILE_EXPR);
ast_node!(FunctionExpr, SyntaxKind::FUNCTION_EXPR);
ast_node!(BlockExpr, SyntaxKind::BLOCK_EXPR);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForExprParts {
    pub leading_comments: Vec<SyntaxToken<RLanguage>>,
    pub variable_elements: Vec<SyntaxElement<RLanguage>>,
    pub sequence_elements: Vec<SyntaxElement<RLanguage>>,
    pub post_clause_comments: Vec<SyntaxToken<RLanguage>>,
    pub body: Option<SyntaxElement<RLanguage>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhileExprParts {
    pub leading_comments: Vec<SyntaxToken<RLanguage>>,
    pub condition_elements: Vec<SyntaxElement<RLanguage>>,
    pub post_clause_comments: Vec<SyntaxToken<RLanguage>>,
    pub body: Option<SyntaxElement<RLanguage>>,
}

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

impl IfExpr {
    pub fn elements(&self) -> Vec<SyntaxElement<RLanguage>> {
        self.syntax().children_with_tokens().collect()
    }

    pub fn if_keyword(&self) -> Option<SyntaxToken<RLanguage>> {
        self.elements()
            .into_iter()
            .find_map(|element| match element {
                SyntaxElement::Token(token) if token.kind() == SyntaxKind::IF_KW => Some(token),
                _ => None,
            })
    }

    pub fn condition_elements(&self) -> Option<Vec<SyntaxElement<RLanguage>>> {
        let elements = self.elements();
        let lparen_idx = self.lparen_index()?;
        let rparen_idx = self.rparen_index()?;
        Some(elements[lparen_idx + 1..rparen_idx].to_vec())
    }

    pub fn then_elements(&self) -> Option<Vec<SyntaxElement<RLanguage>>> {
        let elements = self.elements();
        let rparen_idx = self.rparen_index()?;
        let else_idx = find_token_after_index(&elements, rparen_idx, SyntaxKind::ELSE_KW);
        let then_end = else_idx.unwrap_or(elements.len());
        Some(elements[rparen_idx + 1..then_end].to_vec())
    }

    pub fn else_keyword(&self) -> Option<SyntaxToken<RLanguage>> {
        self.elements()
            .into_iter()
            .find_map(|element| match element {
                SyntaxElement::Token(token) if token.kind() == SyntaxKind::ELSE_KW => Some(token),
                _ => None,
            })
    }

    pub fn else_elements(&self) -> Option<Vec<SyntaxElement<RLanguage>>> {
        let elements = self.elements();
        let else_idx = find_token_index(&elements, SyntaxKind::ELSE_KW)?;
        Some(elements[else_idx + 1..].to_vec())
    }

    pub fn lparen_index(&self) -> Option<usize> {
        find_token_index(&self.elements(), SyntaxKind::LPAREN)
    }

    pub fn rparen_index(&self) -> Option<usize> {
        let elements = self.elements();
        let lparen_idx = self.lparen_index()?;
        find_token_after_index(&elements, lparen_idx, SyntaxKind::RPAREN)
    }
}

impl ForExpr {
    pub fn elements(&self) -> Vec<SyntaxElement<RLanguage>> {
        self.syntax().children_with_tokens().collect()
    }

    pub fn for_keyword(&self) -> Option<SyntaxToken<RLanguage>> {
        self.elements()
            .into_iter()
            .find_map(|element| match element {
                SyntaxElement::Token(token) if token.kind() == SyntaxKind::FOR_KW => Some(token),
                _ => None,
            })
    }

    pub fn clause_bounds(&self) -> Option<(usize, usize)> {
        let elements = self.elements();
        let lparen_idx = self.lparen_index()?;

        let mut depth = 0usize;
        let rparen_idx = elements.iter().enumerate().skip(lparen_idx).find_map(
            |(idx, element)| match element {
                SyntaxElement::Token(token) if token.kind() == SyntaxKind::LPAREN => {
                    depth += 1;
                    None
                }
                SyntaxElement::Token(token) if token.kind() == SyntaxKind::RPAREN => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 { Some(idx) } else { None }
                }
                _ => None,
            },
        )?;

        Some((lparen_idx, rparen_idx))
    }

    pub fn lparen_index(&self) -> Option<usize> {
        let elements = self.elements();
        let for_idx = find_token_index(&elements, SyntaxKind::FOR_KW)?;
        find_token_after_index(&elements, for_idx, SyntaxKind::LPAREN)
    }

    pub fn leading_comments(&self) -> Option<Vec<SyntaxToken<RLanguage>>> {
        let elements = self.elements();
        let for_idx = find_token_index(&elements, SyntaxKind::FOR_KW)?;
        let (_, rparen_idx) = self.clause_bounds()?;
        Some(
            elements[for_idx + 1..rparen_idx]
                .iter()
                .filter_map(|element| match element {
                    SyntaxElement::Token(token) if token.kind() == SyntaxKind::COMMENT => {
                        Some(token.clone())
                    }
                    _ => None,
                })
                .collect(),
        )
    }

    pub fn clause_elements(&self) -> Option<Vec<SyntaxElement<RLanguage>>> {
        let elements = self.elements();
        let (lparen_idx, rparen_idx) = self.clause_bounds()?;
        Some(
            elements[lparen_idx + 1..rparen_idx]
                .iter()
                .filter(|element| {
                    !is_trivia(element.kind()) && element.kind() != SyntaxKind::COMMENT
                })
                .cloned()
                .collect(),
        )
    }

    pub fn post_clause_comments(&self) -> Option<Vec<SyntaxToken<RLanguage>>> {
        let elements = self.elements();
        let (_, rparen_idx) = self.clause_bounds()?;
        let mut comments = Vec::new();
        for element in &elements[rparen_idx + 1..] {
            if is_trivia(element.kind()) {
                continue;
            }
            if let SyntaxElement::Token(token) = element
                && token.kind() == SyntaxKind::COMMENT
            {
                comments.push(token.clone());
                continue;
            }
            break;
        }
        Some(comments)
    }

    pub fn body_element(&self) -> Option<SyntaxElement<RLanguage>> {
        let elements = self.elements();
        let (_, rparen_idx) = self.clause_bounds()?;
        for element in &elements[rparen_idx + 1..] {
            if is_trivia(element.kind()) {
                continue;
            }
            if matches!(element, SyntaxElement::Token(token) if token.kind() == SyntaxKind::COMMENT)
            {
                continue;
            }
            return Some(element.clone());
        }
        None
    }

    pub fn parts(&self) -> Option<ForExprParts> {
        self.for_keyword()?;
        self.lparen_index()?;
        self.clause_bounds()?;

        let clause_elements = self.clause_elements()?;
        let in_idx = clause_elements.iter().position(
            |el| matches!(el, SyntaxElement::Token(tok) if tok.kind() == SyntaxKind::IN_KW),
        )?;

        Some(ForExprParts {
            leading_comments: self.leading_comments()?,
            variable_elements: clause_elements[..in_idx].to_vec(),
            sequence_elements: clause_elements[in_idx + 1..].to_vec(),
            post_clause_comments: self.post_clause_comments()?,
            body: self.body_element(),
        })
    }
}

impl WhileExpr {
    pub fn elements(&self) -> Vec<SyntaxElement<RLanguage>> {
        self.syntax().children_with_tokens().collect()
    }

    pub fn while_keyword(&self) -> Option<SyntaxToken<RLanguage>> {
        self.elements()
            .into_iter()
            .find_map(|element| match element {
                SyntaxElement::Token(token) if token.kind() == SyntaxKind::WHILE_KW => Some(token),
                _ => None,
            })
    }

    pub fn clause_bounds(&self) -> Option<(usize, usize)> {
        let elements = self.elements();
        let lparen_idx = self.lparen_index()?;

        let mut depth = 0usize;
        let rparen_idx = elements.iter().enumerate().skip(lparen_idx).find_map(
            |(idx, element)| match element {
                SyntaxElement::Token(token) if token.kind() == SyntaxKind::LPAREN => {
                    depth += 1;
                    None
                }
                SyntaxElement::Token(token) if token.kind() == SyntaxKind::RPAREN => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 { Some(idx) } else { None }
                }
                _ => None,
            },
        )?;

        Some((lparen_idx, rparen_idx))
    }

    pub fn lparen_index(&self) -> Option<usize> {
        let elements = self.elements();
        let while_idx = find_token_index(&elements, SyntaxKind::WHILE_KW)?;
        find_token_after_index(&elements, while_idx, SyntaxKind::LPAREN)
    }

    pub fn leading_comments(&self) -> Option<Vec<SyntaxToken<RLanguage>>> {
        let elements = self.elements();
        let while_idx = find_token_index(&elements, SyntaxKind::WHILE_KW)?;
        let (_, rparen_idx) = self.clause_bounds()?;
        Some(
            elements[while_idx + 1..rparen_idx]
                .iter()
                .filter_map(|element| match element {
                    SyntaxElement::Token(token) if token.kind() == SyntaxKind::COMMENT => {
                        Some(token.clone())
                    }
                    _ => None,
                })
                .collect(),
        )
    }

    pub fn condition_elements(&self) -> Option<Vec<SyntaxElement<RLanguage>>> {
        let elements = self.elements();
        let (lparen_idx, rparen_idx) = self.clause_bounds()?;
        Some(
            elements[lparen_idx + 1..rparen_idx]
                .iter()
                .filter(|element| {
                    !is_trivia(element.kind()) && element.kind() != SyntaxKind::COMMENT
                })
                .cloned()
                .collect(),
        )
    }

    pub fn post_clause_comments(&self) -> Option<Vec<SyntaxToken<RLanguage>>> {
        let elements = self.elements();
        let (_, rparen_idx) = self.clause_bounds()?;
        let mut comments = Vec::new();
        for element in &elements[rparen_idx + 1..] {
            if is_trivia(element.kind()) {
                continue;
            }
            if let SyntaxElement::Token(token) = element
                && token.kind() == SyntaxKind::COMMENT
            {
                comments.push(token.clone());
                continue;
            }
            break;
        }
        Some(comments)
    }

    pub fn body_element(&self) -> Option<SyntaxElement<RLanguage>> {
        let elements = self.elements();
        let (_, rparen_idx) = self.clause_bounds()?;
        for element in &elements[rparen_idx + 1..] {
            if is_trivia(element.kind()) {
                continue;
            }
            if matches!(element, SyntaxElement::Token(token) if token.kind() == SyntaxKind::COMMENT)
            {
                continue;
            }
            return Some(element.clone());
        }
        None
    }

    pub fn parts(&self) -> Option<WhileExprParts> {
        self.while_keyword()?;
        self.lparen_index()?;
        self.clause_bounds()?;

        Some(WhileExprParts {
            leading_comments: self.leading_comments()?,
            condition_elements: self.condition_elements()?,
            post_clause_comments: self.post_clause_comments()?,
            body: self.body_element(),
        })
    }
}

fn find_token_index(elements: &[SyntaxElement<RLanguage>], kind: SyntaxKind) -> Option<usize> {
    elements
        .iter()
        .position(|element| matches!(element, SyntaxElement::Token(token) if token.kind() == kind))
}

fn find_token_after_index(
    elements: &[SyntaxElement<RLanguage>],
    start_idx: usize,
    kind: SyntaxKind,
) -> Option<usize> {
    elements
        .iter()
        .enumerate()
        .skip(start_idx + 1)
        .find_map(|(idx, element)| match element {
            SyntaxElement::Token(token) if token.kind() == kind => Some(idx),
            _ => None,
        })
}

fn is_trivia(kind: SyntaxKind) -> bool {
    matches!(kind, SyntaxKind::WHITESPACE | SyntaxKind::NEWLINE)
}

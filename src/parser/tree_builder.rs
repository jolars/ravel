use rowan::GreenNodeBuilder;

use crate::parser::events::Event;
use crate::parser::lexer::{TokKind, Token};
use crate::syntax::{SyntaxKind, SyntaxNode};

pub(crate) fn build_tree(tokens: &[Token], events: &[Event]) -> SyntaxNode {
    let mut builder = GreenNodeBuilder::new();
    builder.start_node(SyntaxKind::ROOT.into());

    for event in events {
        match *event {
            Event::Start(kind) => builder.start_node(kind.into()),
            Event::Tok(idx) => push_token(&mut builder, &tokens[idx]),
            Event::Finish => builder.finish_node(),
        }
    }

    builder.finish_node();
    let green = builder.finish();
    SyntaxNode::new_root(green)
}

fn push_token(builder: &mut GreenNodeBuilder<'_>, tok: &Token) {
    let sk = match tok.kind {
        TokKind::Ident => SyntaxKind::IDENT,
        TokKind::Int => SyntaxKind::INT,
        TokKind::Float => SyntaxKind::FLOAT,
        TokKind::String => SyntaxKind::STRING,
        TokKind::Comment => SyntaxKind::COMMENT,
        TokKind::UserOp => SyntaxKind::USER_OP,
        TokKind::LBrack2 => SyntaxKind::LBRACK2,
        TokKind::RBrack2 => SyntaxKind::RBRACK2,
        TokKind::Plus => SyntaxKind::PLUS,
        TokKind::Star => SyntaxKind::STAR,
        TokKind::Caret => SyntaxKind::CARET,
        TokKind::LParen => SyntaxKind::LPAREN,
        TokKind::RParen => SyntaxKind::RPAREN,
        TokKind::IfKw => SyntaxKind::IF_KW,
        TokKind::ElseKw => SyntaxKind::ELSE_KW,
        TokKind::ForKw => SyntaxKind::FOR_KW,
        TokKind::WhileKw => SyntaxKind::WHILE_KW,
        TokKind::FunctionKw => SyntaxKind::FUNCTION_KW,
        TokKind::InKw => SyntaxKind::IN_KW,
        TokKind::LBrace => SyntaxKind::LBRACE,
        TokKind::RBrace => SyntaxKind::RBRACE,
        TokKind::AssignLeft => SyntaxKind::ASSIGN_LEFT,
        TokKind::Whitespace => SyntaxKind::WHITESPACE,
        TokKind::Newline => SyntaxKind::NEWLINE,
        TokKind::Unknown => SyntaxKind::ERROR,
    };
    builder.token(sk.into(), tok.text.as_str());
}

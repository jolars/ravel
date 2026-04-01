use rowan::{NodeOrToken, SyntaxElement};

use super::core::FormatError;
use crate::syntax::{RLanguage, SyntaxKind};

pub(super) fn split_lines(
    elements: Vec<SyntaxElement<RLanguage>>,
    context: &'static str,
) -> Result<Vec<Vec<SyntaxElement<RLanguage>>>, FormatError> {
    let mut lines: Vec<Vec<SyntaxElement<RLanguage>>> = Vec::new();
    let mut current: Vec<SyntaxElement<RLanguage>> = Vec::new();
    let mut break_count = 0usize;

    for element in elements {
        if let NodeOrToken::Token(token) = &element {
            if token.kind() == SyntaxKind::WHITESPACE {
                continue;
            }
            if token.kind() == SyntaxKind::NEWLINE || token.kind() == SyntaxKind::SEMICOLON {
                if !current.is_empty() {
                    lines.push(std::mem::take(&mut current));
                    break_count = 1;
                } else if !lines.is_empty() {
                    break_count += 1;
                }
                continue;
            }
        }

        if break_count >= 2
            && (!matches!(lines.last(), Some(last) if is_comment_only_line(last))
                || matches!(element, NodeOrToken::Token(ref tok) if tok.kind() == SyntaxKind::COMMENT))
        {
            lines.push(Vec::new());
        }
        break_count = 0;

        if !current.is_empty() {
            if matches!(element, NodeOrToken::Token(ref tok) if tok.kind() == SyntaxKind::COMMENT)
                && !current.iter().any(
                    |el| matches!(el, NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::COMMENT),
                )
            {
                current.push(element);
                continue;
            }
            return Err(FormatError::AmbiguousConstruct {
                context,
                snippet: super::render::snippet_from_elements(&[current[0].clone(), element]),
            });
        }
        current.push(element);
    }

    if !current.is_empty() {
        lines.push(current);
    }

    Ok(lines)
}

pub(super) fn is_trivia(kind: SyntaxKind) -> bool {
    matches!(kind, SyntaxKind::WHITESPACE | SyntaxKind::NEWLINE)
}

fn is_comment_only_line(line: &[SyntaxElement<RLanguage>]) -> bool {
    let significant: Vec<_> = line.iter().filter(|el| !is_trivia(el.kind())).collect();
    matches!(
        significant.as_slice(),
        [NodeOrToken::Token(tok)] if tok.kind() == SyntaxKind::COMMENT
    )
}

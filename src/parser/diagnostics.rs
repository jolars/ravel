use crate::parser::lexer::Token;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseDiagnostic {
    pub message: String,
    pub start: usize,
    pub end: usize,
}

pub(crate) fn push_diagnostic(
    diagnostics: &mut Vec<ParseDiagnostic>,
    message: &str,
    start: usize,
    end: usize,
) {
    diagnostics.push(ParseDiagnostic {
        message: message.to_string(),
        start,
        end,
    });
}

pub(crate) fn push_token_diagnostic(
    diagnostics: &mut Vec<ParseDiagnostic>,
    message: &str,
    token: &Token,
) {
    push_diagnostic(diagnostics, message, token.start, token.end);
}

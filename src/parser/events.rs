use crate::syntax::SyntaxKind;

#[derive(Debug, Clone)]
pub(crate) enum Event {
    Start(SyntaxKind),
    Tok(usize),
    Finish,
}

#[derive(Debug, Clone)]
pub(crate) struct ExprParse {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) events: Vec<Event>,
}

pub(crate) fn push_range(events: &mut Vec<Event>, start: usize, end: usize) {
    for idx in start..end {
        events.push(Event::Tok(idx));
    }
}

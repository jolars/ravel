//! A lightweight Wadler/Prettier-style intermediate representation (IR) for the
//! formatter.
//!
//! Construct formatters build an [`Ir`] tree describing *possible* layouts (with
//! break-points), and [`super::printer::Printer`] resolves it against the
//! configured line width into a final string. This replaces the older model
//! where each construct rendered directly to a `String` and width was measured
//! retrospectively.

// Builders are introduced ahead of their consumers during the incremental
// migration to the IR; remove this allow in the final cleanup step.
#![allow(dead_code)]

use std::rc::Rc;

/// A document node describing how a piece of code may be laid out.
#[derive(Debug, Clone)]
pub(crate) enum Ir {
    /// Literal text. Must never contain a newline.
    Text(Rc<str>),
    /// A sequence of nodes printed back-to-back.
    Concat(Rc<[Ir]>),
    /// Flat mode: a single space. Break mode: newline + current indent.
    Line,
    /// Flat mode: nothing. Break mode: newline + current indent.
    SoftLine,
    /// Always a newline + current indent, regardless of mode. Forces every
    /// enclosing [`Ir::Group`] to break.
    HardLine,
    /// A blank line followed by the next line's indent. Like [`Ir::HardLine`] it
    /// forces enclosing groups to break.
    EmptyLine,
    /// Increase the indent of everything inside by one `indent_width` step.
    Indent(Rc<Ir>),
    /// A break-decision boundary. The printer measures the flat rendering of
    /// `inner`; if it fits and contains no forced break, it prints flat,
    /// otherwise broken. `expand` forces broken unconditionally.
    Group { inner: Rc<Ir>, expand: bool },
    /// Emit `flat` when the enclosing group is flat, `broken` when it is broken.
    IfBreak { flat: Rc<Ir>, broken: Rc<Ir> },
    /// Pre-rendered text (comments, or not-yet-migrated constructs) whose
    /// internal newlines are re-indented to the current column but otherwise
    /// passed through untouched. Forces enclosing groups to break.
    Verbatim(Rc<str>),
    /// Nothing.
    Nil,
}

impl Ir {
    pub(crate) fn text(s: impl Into<Rc<str>>) -> Ir {
        Ir::Text(s.into())
    }

    pub(crate) fn concat(items: impl IntoIterator<Item = Ir>) -> Ir {
        let items: Vec<Ir> = items
            .into_iter()
            .filter(|i| !matches!(i, Ir::Nil))
            .collect();
        match items.len() {
            0 => Ir::Nil,
            1 => items.into_iter().next().unwrap(),
            _ => Ir::Concat(items.into()),
        }
    }

    /// Interleave `items` with `sep`.
    pub(crate) fn join(sep: Ir, items: impl IntoIterator<Item = Ir>) -> Ir {
        let mut out = Vec::new();
        for (i, item) in items.into_iter().enumerate() {
            if i > 0 {
                out.push(sep.clone());
            }
            out.push(item);
        }
        Ir::concat(out)
    }

    pub(crate) fn group(inner: Ir) -> Ir {
        Ir::Group {
            inner: Rc::new(inner),
            expand: false,
        }
    }

    pub(crate) fn group_expanded(inner: Ir) -> Ir {
        Ir::Group {
            inner: Rc::new(inner),
            expand: true,
        }
    }

    pub(crate) fn indent(inner: Ir) -> Ir {
        Ir::Indent(Rc::new(inner))
    }

    pub(crate) fn if_break(flat: Ir, broken: Ir) -> Ir {
        Ir::IfBreak {
            flat: Rc::new(flat),
            broken: Rc::new(broken),
        }
    }

    pub(crate) fn verbatim(s: impl Into<Rc<str>>) -> Ir {
        Ir::Verbatim(s.into())
    }

    pub(crate) fn line() -> Ir {
        Ir::Line
    }

    pub(crate) fn soft_line() -> Ir {
        Ir::SoftLine
    }

    pub(crate) fn hard_line() -> Ir {
        Ir::HardLine
    }

    pub(crate) fn empty_line() -> Ir {
        Ir::EmptyLine
    }

    pub(crate) fn nil() -> Ir {
        Ir::Nil
    }
}

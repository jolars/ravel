//! The layout engine: walks an [`Ir`] tree and renders it to a string, deciding
//! for each [`Ir::Group`] whether it fits flat on the current line or must break.

use super::ir::Ir;
use super::style::FormatStyle;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Flat,
    Break,
}

pub(crate) struct Printer {
    line_width: usize,
    indent_unit: usize,
}

/// Accumulates output while deferring indentation until visible content is
/// written, so blank lines never carry trailing whitespace.
struct Writer {
    out: String,
    col: usize,
    pending_indent: usize,
    needs_indent: bool,
}

impl Writer {
    fn new() -> Self {
        Self {
            out: String::new(),
            col: 0,
            pending_indent: 0,
            needs_indent: false,
        }
    }

    fn flush_indent(&mut self) {
        if self.needs_indent {
            for _ in 0..self.pending_indent {
                self.out.push(' ');
            }
            self.col += self.pending_indent;
            self.needs_indent = false;
        }
    }

    /// Write text that contains no newline.
    fn write_text(&mut self, s: &str) {
        if s.is_empty() {
            return;
        }
        self.flush_indent();
        self.out.push_str(s);
        self.col += s.chars().count();
    }

    /// Move to a fresh line indented to `indent`.
    fn newline(&mut self, indent: usize) {
        self.out.push('\n');
        self.col = 0;
        self.pending_indent = indent;
        self.needs_indent = true;
    }

    /// Emit a blank line, then position on a fresh line indented to `indent`.
    fn empty_line(&mut self, indent: usize) {
        self.out.push('\n');
        self.out.push('\n');
        self.col = 0;
        self.pending_indent = indent;
        self.needs_indent = true;
    }

    /// Splice a possibly multi-line string verbatim. The string is assumed to
    /// already carry its own indentation, so only a pending indent on the very
    /// first line is honored.
    fn write_verbatim(&mut self, s: &str) {
        let mut first = true;
        for segment in s.split('\n') {
            if first {
                self.flush_indent();
                first = false;
            } else {
                self.out.push('\n');
                self.col = 0;
                self.needs_indent = false;
            }
            self.out.push_str(segment);
            self.col += segment.chars().count();
        }
    }
}

impl Printer {
    pub(crate) fn new(style: FormatStyle) -> Self {
        Self {
            line_width: style.line_width,
            indent_unit: style.indent_width,
        }
    }

    /// Print a complete document starting at column 0.
    pub(crate) fn print(&self, ir: &Ir) -> String {
        self.run(ir, 0, 0)
    }

    /// Print an expression that will be placed at indent level `indent_level`,
    /// without emitting the leading indent on the first line (the caller does
    /// that). The starting column accounts for the indent so width decisions
    /// match where the expression actually sits.
    pub(crate) fn print_at(&self, ir: &Ir, indent_level: usize) -> String {
        let base = indent_level * self.indent_unit;
        self.run(ir, base, base)
    }

    fn run(&self, ir: &Ir, base_indent: usize, init_col: usize) -> String {
        let mut w = Writer::new();
        w.col = init_col;
        let mut stack: Vec<(usize, Mode, &Ir)> = vec![(base_indent, Mode::Break, ir)];
        while let Some((indent, mode, node)) = stack.pop() {
            match node {
                Ir::Nil => {}
                Ir::Text(s) => w.write_text(s),
                Ir::Verbatim { text, .. } => w.write_verbatim(text),
                Ir::Concat(items) => {
                    for item in items.iter().rev() {
                        stack.push((indent, mode, item));
                    }
                }
                Ir::Indent(inner) => {
                    stack.push((indent + self.indent_unit, mode, inner));
                }
                Ir::Line => match mode {
                    Mode::Flat => w.write_text(" "),
                    Mode::Break => w.newline(indent),
                },
                Ir::SoftLine => {
                    if mode == Mode::Break {
                        w.newline(indent);
                    }
                }
                Ir::HardLine => w.newline(indent),
                Ir::EmptyLine => w.empty_line(indent),
                Ir::IfBreak { flat, broken } => {
                    let chosen = if mode == Mode::Break { broken } else { flat };
                    stack.push((indent, mode, chosen));
                }
                Ir::Group { inner, expand } => {
                    let m = if *expand || !self.fits(w.col, inner) {
                        Mode::Break
                    } else {
                        Mode::Flat
                    };
                    stack.push((indent, m, inner));
                }
            }
        }
        w.out
    }

    /// Simulate `node` flat, starting at column `start_col`. Returns false on the
    /// first forced break or as soon as the running width exceeds the line.
    fn fits(&self, start_col: usize, node: &Ir) -> bool {
        let mut remaining = self.line_width.saturating_sub(start_col);
        let mut stack: Vec<&Ir> = vec![node];
        while let Some(node) = stack.pop() {
            match node {
                Ir::Nil | Ir::SoftLine => {}
                Ir::Text(s) => {
                    let w = s.chars().count();
                    if w > remaining {
                        return false;
                    }
                    remaining -= w;
                }
                Ir::HardLine | Ir::EmptyLine => return false,
                Ir::Verbatim { text, force_break } => {
                    if *force_break {
                        return false;
                    }
                    let w = text.chars().count();
                    if w > remaining {
                        return false;
                    }
                    remaining -= w;
                }
                Ir::Concat(items) => {
                    for item in items.iter().rev() {
                        stack.push(item);
                    }
                }
                Ir::Indent(inner) => stack.push(inner),
                Ir::Line => {
                    if remaining == 0 {
                        return false;
                    }
                    remaining -= 1;
                }
                Ir::IfBreak { flat, .. } => stack.push(flat),
                Ir::Group { inner, expand } => {
                    if *expand {
                        return false;
                    }
                    stack.push(inner);
                }
            }
        }
        true
    }
}

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
                Ir::Group { inner, expand, hug } => {
                    let m = if *expand || !self.fits(w.col, inner, *hug) {
                        Mode::Break
                    } else {
                        Mode::Flat
                    };
                    stack.push((indent, m, inner));
                }
                Ir::ConditionalGroup(cands) => {
                    let (m, chosen) = self.pick_candidate(w.col, cands);
                    stack.push((indent, m, chosen));
                }
            }
        }
        w.out
    }

    /// Pick the layout for an [`Ir::ConditionalGroup`] at the current column:
    /// the first candidate whose first line fits is rendered flat; if none, the
    /// last candidate is rendered broken. With a single candidate this is a
    /// "break-aware group" — flat if its first line fits, broken otherwise.
    fn pick_candidate<'a>(&self, col: usize, cands: &'a [Ir]) -> (Mode, &'a Ir) {
        let n = cands.len();
        for (i, c) in cands.iter().enumerate() {
            if self.first_line_fits(col, c) {
                return (Mode::Flat, c);
            }
            if i + 1 == n {
                return (Mode::Break, c);
            }
        }
        unreachable!("Ir::ConditionalGroup builder rejects empty candidate lists")
    }

    /// Simulate `node` flat, starting at column `start_col`. Returns false on the
    /// first forced break or as soon as the running width exceeds the line.
    ///
    /// When `hug` is set, a forced line break (`HardLine`/`EmptyLine`) instead
    /// stops the measurement *successfully*: only the prefix up to a trailing
    /// block's opening brace needs to fit. A forced-break `Verbatim` (a comment)
    /// still fails, so a comment in the prefix prevents hugging.
    fn fits(&self, start_col: usize, node: &Ir, hug: bool) -> bool {
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
                Ir::HardLine | Ir::EmptyLine => return hug,
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
                Ir::Group { inner, expand, .. } => {
                    if *expand {
                        return false;
                    }
                    stack.push(inner);
                }
                // Conservative: measure as the flat-most candidate. A nested
                // conditional group inside a flat measurement is rare today
                // (the only producer is the trailing-function call hug); if
                // and when one nests, this matches the most permissive layout.
                Ir::ConditionalGroup(cands) => {
                    if let Some(first) = cands.first() {
                        stack.push(first);
                    }
                }
            }
        }
        true
    }

    /// Does the *first line* of `node` fit starting at `start_col`? Unlike
    /// [`Self::fits`] (a flat simulation), this lets nested [`Ir::Group`]s
    /// decide their own break naturally — they re-use the existing flat
    /// `fits` exactly as the real printer does — and treats the first
    /// newline that would actually be emitted (a `HardLine`/`EmptyLine`, a
    /// `Line`/`SoftLine` in `Break` mode, or anything in a nested group
    /// decided `Break`) as success. A forced-break `Verbatim` fails, since
    /// the candidate cannot be rendered flat at all.
    fn first_line_fits(&self, start_col: usize, node: &Ir) -> bool {
        let mut col = start_col;
        let mut stack: Vec<(Mode, &Ir)> = vec![(Mode::Flat, node)];
        while let Some((mode, node)) = stack.pop() {
            match node {
                Ir::Nil => {}
                Ir::Text(s) => {
                    col += s.chars().count();
                    if col > self.line_width {
                        return false;
                    }
                }
                Ir::Verbatim { text, force_break } => {
                    if *force_break {
                        return false;
                    }
                    col += text.chars().count();
                    if col > self.line_width {
                        return false;
                    }
                }
                Ir::Concat(items) => {
                    for item in items.iter().rev() {
                        stack.push((mode, item));
                    }
                }
                Ir::Indent(inner) => stack.push((mode, inner)),
                Ir::Line => match mode {
                    Mode::Flat => {
                        col += 1;
                        if col > self.line_width {
                            return false;
                        }
                    }
                    Mode::Break => return true,
                },
                Ir::SoftLine => {
                    if mode == Mode::Break {
                        return true;
                    }
                }
                Ir::HardLine | Ir::EmptyLine => return true,
                Ir::IfBreak { flat, broken } => {
                    let chosen = if mode == Mode::Break { broken } else { flat };
                    stack.push((mode, chosen));
                }
                Ir::Group { inner, expand, hug } => {
                    let m = if *expand || !self.fits(col, inner, *hug) {
                        Mode::Break
                    } else {
                        Mode::Flat
                    };
                    stack.push((m, inner));
                }
                Ir::ConditionalGroup(cands) => {
                    let (m, chosen) = self.pick_candidate(col, cands);
                    stack.push((m, chosen));
                }
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A block that always breaks: `{`, an indented body, then `}`.
    fn block() -> Ir {
        Ir::concat([
            Ir::text("{"),
            Ir::indent(Ir::concat([Ir::hard_line(), Ir::text("body")])),
            Ir::hard_line(),
            Ir::text("}"),
        ])
    }

    /// `f(a, {block})` as a hug group: prefix `f(a, ` then a trailing block.
    fn hug_call() -> Ir {
        Ir::group_hug(Ir::concat([
            Ir::text("f("),
            Ir::indent(Ir::concat([
                Ir::soft_line(),
                Ir::text("a"),
                Ir::if_break(Ir::text(", "), Ir::text(",")),
            ])),
            Ir::if_break(block(), Ir::indent(Ir::concat([Ir::soft_line(), block()]))),
            Ir::soft_line(),
            Ir::text(")"),
        ]))
    }

    #[test]
    fn hug_group_keeps_prefix_flat_when_it_fits() {
        let printer = Printer::new(FormatStyle::default());
        assert_eq!(printer.print(&hug_call()), "f(a, {\n  body\n})");
    }

    #[test]
    fn hug_group_expands_when_prefix_does_not_fit() {
        // A narrow line forces even the short prefix `f(a, {` to break.
        let style = FormatStyle {
            line_width: 5,
            indent_width: 2,
        };
        let printer = Printer::new(style);
        assert_eq!(
            printer.print(&hug_call()),
            "f(\n  a,\n  {\n    body\n  }\n)"
        );
    }

    #[test]
    fn hug_group_expands_when_prefix_has_a_comment() {
        // A forced-break verbatim (a comment) in the prefix prevents hugging
        // even though the prefix is short.
        let printer = Printer::new(FormatStyle::default());
        let ir = Ir::group_hug(Ir::concat([
            Ir::text("f("),
            Ir::indent(Ir::concat([
                Ir::soft_line(),
                Ir::verbatim_forced("# c"),
                Ir::hard_line(),
                Ir::text("a"),
                Ir::if_break(Ir::text(", "), Ir::text(",")),
            ])),
            Ir::if_break(block(), Ir::indent(Ir::concat([Ir::soft_line(), block()]))),
            Ir::soft_line(),
            Ir::text(")"),
        ]));
        // Expanded: the comment lands on its own line and the block is indented.
        assert_eq!(printer.print(&ir), "f(\n  # c\n  a,\n  {\n    body\n  }\n)");
    }

    /// A nested group whose flat form overflows the line but whose own break
    /// emits a newline before the overflow point. The conditional group's
    /// first-line measurement lets the nested group break, so the outer line
    /// fits even though the inner cannot stay flat.
    fn nested_breakable_group(width: usize) -> Ir {
        let long = "x".repeat(width);
        // Inner group: flat = `(<long>)` (overflows at width ≥ ~outer.width);
        // broken = `(\n  <long>\n)`.
        let inner = Ir::group(Ir::concat([
            Ir::text("("),
            Ir::indent(Ir::concat([Ir::soft_line(), Ir::text(long)])),
            Ir::soft_line(),
            Ir::text(")"),
        ]));
        // Outer candidate: `f` then the inner group. Its first line is `f(`.
        Ir::concat([Ir::text("f"), inner])
    }

    #[test]
    fn conditional_group_single_candidate_flat_when_first_line_fits() {
        // The inner group cannot fit flat (long >> width), but the conditional
        // group's first-line measurement lets it break naturally: `f(` fits
        // and the inner emits its own newline.
        let style = FormatStyle {
            line_width: 10,
            indent_width: 2,
        };
        let printer = Printer::new(style);
        let ir = Ir::conditional_group([nested_breakable_group(20)]);
        assert_eq!(printer.print(&ir), "f(\n  xxxxxxxxxxxxxxxxxxxx\n)");
    }

    #[test]
    fn conditional_group_single_candidate_breaks_when_first_line_does_not_fit() {
        // A long literal in the candidate's first line itself blows the budget
        // before any nested group can break: fall to Break mode for the same
        // (single) candidate.
        let style = FormatStyle {
            line_width: 5,
            indent_width: 2,
        };
        let printer = Printer::new(style);
        // Candidate: `verylong` then a Line. In Flat: `verylong ` overflows;
        // in Break: the Line becomes a newline.
        let ir = Ir::conditional_group([Ir::concat([
            Ir::text("verylong"),
            Ir::line(),
            Ir::text("x"),
        ])]);
        assert_eq!(printer.print(&ir), "verylong\nx");
    }

    #[test]
    fn conditional_group_picks_first_fitting_candidate() {
        let style = FormatStyle {
            line_width: 6,
            indent_width: 2,
        };
        let printer = Printer::new(style);
        // c0 doesn't fit; c1 fits; c2 (fallback) never reached.
        let c0 = Ir::text("toolongtofit");
        let c1 = Ir::text("ok");
        let c2 = Ir::concat([Ir::text("fallback"), Ir::hard_line(), Ir::text("more")]);
        let ir = Ir::conditional_group([c0, c1, c2]);
        assert_eq!(printer.print(&ir), "ok");
    }

    #[test]
    fn conditional_group_falls_back_to_last_in_break_mode() {
        let style = FormatStyle {
            line_width: 4,
            indent_width: 2,
        };
        let printer = Printer::new(style);
        // Neither earlier candidate fits; the last is rendered broken (its
        // `Line` becomes a newline).
        let c0 = Ir::text("toolongtofit");
        let c1 = Ir::text("alsotoolong");
        let c2 = Ir::concat([Ir::text("ab"), Ir::line(), Ir::text("cd")]);
        let ir = Ir::conditional_group([c0, c1, c2]);
        assert_eq!(printer.print(&ir), "ab\ncd");
    }
}

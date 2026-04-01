use super::style::FormatStyle;

#[derive(Debug, Clone, Copy)]
pub(crate) struct FormatContext {
    style: FormatStyle,
}

impl FormatContext {
    pub(crate) fn new(style: FormatStyle) -> Self {
        Self { style }
    }

    pub(crate) fn indent_text(self, indent: usize) -> String {
        " ".repeat(self.style.indent_width * indent)
    }

    pub(crate) fn fits_inline(self, indent: usize, text: &str) -> bool {
        !text.contains('\n') && text.chars().count() <= self.max_inline_width(indent)
    }

    pub(crate) fn fits_with_newlines(self, indent: usize, text: &str) -> bool {
        let max = self.max_inline_width(indent);
        text.lines().all(|line| line.chars().count() <= max)
    }

    fn max_inline_width(self, indent: usize) -> usize {
        self.style
            .line_width
            .saturating_sub(self.style.indent_width * indent)
    }
}

pub mod check;
pub(crate) mod context;
pub mod core;
pub(crate) mod render;
pub(crate) mod rules;
pub mod style;
pub(crate) mod trivia;

pub use check::{CheckError, CheckResult, check_paths};
pub use core::{FormatError, format, format_with_style};
pub use style::FormatStyle;

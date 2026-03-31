pub mod check;
pub(crate) mod context;
pub mod core;
pub mod style;

pub use check::{CheckError, CheckResult, check_paths};
pub use core::{FormatError, format, format_with_style};
pub use style::FormatStyle;

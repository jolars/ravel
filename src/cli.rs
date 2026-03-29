use std::path::PathBuf;

use clap::builder::Styles;
use clap::builder::styling::{AnsiColor, Effects};
use clap::{Parser, Subcommand};

const STYLES: Styles = Styles::styled()
    .header(AnsiColor::Green.on_default().effects(Effects::BOLD))
    .usage(AnsiColor::Green.on_default().effects(Effects::BOLD))
    .literal(AnsiColor::Cyan.on_default().effects(Effects::BOLD))
    .placeholder(AnsiColor::Cyan.on_default());

#[derive(Parser)]
#[command(name = "ravel")]
#[command(author, version)]
#[command(about = "Ravel: a language server, formatter, and linter for R")]
#[command(styles = STYLES)]
#[command(arg_required_else_help = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Parse and display the CST tree for debugging
    Parse {
        /// Input file (stdin if not provided)
        file: Option<PathBuf>,

        /// Suppress CST output to stdout
        #[arg(long)]
        quiet: bool,

        /// Verify parser losslessness (input must equal CST text)
        #[arg(long)]
        verify: bool,
    },
}

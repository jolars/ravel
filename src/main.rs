use std::fs;
use std::io::{self, Read};
use std::process::ExitCode;

use clap::Parser;
use ravel::cli::{Cli, Commands};
use ravel::parser::{debug_tree, reconstruct};

fn main() -> ExitCode {
    let cli = Cli::parse();

    match cli.command {
        Commands::Parse {
            file,
            quiet,
            verify,
        } => run_parse(file, quiet, verify),
    }
}

fn run_parse(file: Option<std::path::PathBuf>, quiet: bool, verify: bool) -> ExitCode {
    let input = match read_input(file.as_deref()) {
        Ok(input) => input,
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::from(2);
        }
    };

    if !quiet {
        println!("{}", debug_tree(&input));
    }

    if verify {
        let reconstructed = reconstruct(&input);
        if reconstructed != input {
            eprintln!("error: parser losslessness check failed");
            return ExitCode::from(1);
        }
    }

    ExitCode::SUCCESS
}

fn read_input(path: Option<&std::path::Path>) -> io::Result<String> {
    match path {
        Some(path) => fs::read_to_string(path),
        None => {
            let mut input = String::new();
            io::stdin().read_to_string(&mut input)?;
            Ok(input)
        }
    }
}

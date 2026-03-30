use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use ravel::cli::{Cli, Commands};
use ravel::formatter::format;
use ravel::parser::{parse, reconstruct};

fn main() -> ExitCode {
    let cli = Cli::parse();

    match cli.command {
        Commands::Parse {
            file,
            quiet,
            verify,
        } => run_parse(file, quiet, verify),
        Commands::Format {
            paths,
            verify,
            check,
        } => run_format(paths, verify, check),
        Commands::Lint { paths, check } => run_lint(paths, check),
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

    let parse_output = parse(&input);

    if !quiet {
        println!("{:#?}", parse_output.cst);
    }

    if !parse_output.diagnostics.is_empty() {
        for diag in &parse_output.diagnostics {
            eprintln!("error[{}..{}]: {}", diag.start, diag.end, diag.message);
        }
        return ExitCode::from(1);
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

fn run_format(paths: Vec<PathBuf>, verify: bool, check: bool) -> ExitCode {
    if check {
        if verify {
            eprintln!("error: --verify cannot be combined with --check");
            return ExitCode::from(2);
        }
        return run_format_check(&paths);
    }

    if paths.len() > 1 {
        eprintln!("error: format accepts at most one input path unless --check is used");
        return ExitCode::from(2);
    }

    let input = match read_input(paths.first().map(PathBuf::as_path)) {
        Ok(input) => input,
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::from(2);
        }
    };

    let formatted = match format(&input) {
        Ok(formatted) => formatted,
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::from(1);
        }
    };

    if verify {
        let reformatted = match format(&formatted) {
            Ok(reformatted) => reformatted,
            Err(err) => {
                eprintln!("error: formatted output failed verification: {err}");
                return ExitCode::from(1);
            }
        };
        if reformatted != formatted {
            eprintln!("error: formatter verification failed (non-idempotent output)");
            return ExitCode::from(1);
        }
    }

    print!("{formatted}");
    ExitCode::SUCCESS
}

fn run_format_check(paths: &[PathBuf]) -> ExitCode {
    match ravel::formatter::check_paths(paths) {
        Ok(result) => {
            if result.changed_files.is_empty() {
                ExitCode::SUCCESS
            } else {
                for path in result.changed_files {
                    eprintln!("would reformat: {}", path.display());
                }
                ExitCode::from(1)
            }
        }
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::from(2)
        }
    }
}

fn run_lint(paths: Vec<PathBuf>, check: bool) -> ExitCode {
    if !check {
        eprintln!(
            "error: lint currently requires --check while lint rules are not implemented yet"
        );
        return ExitCode::from(2);
    }

    match ravel::linter::check_paths(&paths) {
        Ok(result) => {
            for report in result.reports {
                match report.status {
                    ravel::linter::LintStatus::RulesNotImplemented => {
                        eprintln!(
                            "lint not yet implemented: {} (parsed successfully)",
                            report.path.display()
                        );
                    }
                    ravel::linter::LintStatus::ParseDiagnostics { count } => {
                        eprintln!(
                            "lint blocked by parse diagnostics: {} ({} diagnostic{})",
                            report.path.display(),
                            count,
                            if count == 1 { "" } else { "s" }
                        );
                    }
                }
            }
            ExitCode::from(1)
        }
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::from(2)
        }
    }
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

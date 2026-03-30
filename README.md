# Ravel

## Formatter

Ravel includes formatting via:

- `ravel format [file]`
- `ravel format --verify [file]`
- `ravel format --check <path> [<path> ...]`

Current behavior:

- supports parseable assignment, binary-expression, parenthesized, `if`/`else`, and block inputs
- normalizes expression spacing and block layout with stable/idempotent output
- preserves comments and string literal contents
- rejects unsupported/ambiguous constructs (for example `%...%`, `[[`, `]]`) with explicit errors
- `--check` scans provided file/directory paths for `.R` files and exits non-zero when any file would be reformatted

## Linter

Ravel includes linting via:

- `ravel lint --check <path> [<path> ...]`

Current behavior:

- reuses `.R` file discovery and parser/incremental plumbing
- parses each discovered `.R` file
- implements `assignment-spacing`: reports when `<-` is not surrounded by exactly one space on each side
  - reports: `x<-1`, `x  <-1`, `x<- 1`
  - passes: `x <- 1`
- emits deterministic diagnostics as `<path>:<line>:<column>: [assignment-spacing] ... (span <start>..<end>)`
- reports one of:
  - lint findings for violated rules
  - `lint blocked by parse diagnostics: <file> (...)` when parsing fails
- exits non-zero when lint findings exist or parsing blocks linting

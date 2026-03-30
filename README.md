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

## Linter (skeleton)

Ravel includes an explicit lint skeleton via:

- `ravel lint --check <path> [<path> ...]`

Current behavior:

- reuses `.R` file discovery and parser/incremental plumbing
- parses each discovered `.R` file
- reports one of:
  - `lint blocked by parse diagnostics: <file> (...)` when parsing fails
  - `lint not yet implemented: <file> (parsed successfully)` when parsing succeeds
- exits non-zero until lint rules are implemented

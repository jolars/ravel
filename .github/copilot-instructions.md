# Copilot instructions for `ravel`

## Build, test, and lint commands

- Build (dev): `cargo build --verbose`
- Build (release): `cargo build --release`
- Run tests: `cargo test` (CI uses `cargo test --verbose`)
- Run one test by name: `cargo test <test_name_substring>`
- Run parser snapshot tests only: `cargo test --test parser_snapshots`
- Run parse CLI on a file: `cargo run -- parse <path/to/file.R>`
- Parse stdin and verify losslessness:
  `cat file.R | cargo run -- parse --verify --quiet`
- Run tests with logs:
  - Debug: `RUST_LOG=debug cargo test`
  - Parser trace: `RUST_LOG=panache::parser=trace cargo test`
  - Quiet logs: `RUST_LOG=off cargo test`
- Review/accept `insta` snapshots:
  - `cargo insta review`
  - `cargo insta accept`
- Lint: `cargo clippy -- -D warnings`
- Format check: `cargo fmt -- --check`
- Docs checks used in CI: `cargo run -- format --check docs/` and
  `cargo run -- lint --check docs/`

Taskfile equivalents:

- `task lint`
- `task format`
- `task test`
- `task test-debug`
- `task test-trace`
- `task test-quiet`

## High-level architecture

- CLI entrypoint is `src/main.rs`, which wires `ravel parse`, `ravel format`,
  and `ravel lint` to library modules in `src/`.
- The repo ships a `devenv` setup (see `devenv.*`) and the Nix development
  environment includes `R`.
- The `air` tree-sitter-based parser is available locally for reference and
  comparison; it is not exposed via this repo's CLI.
- Parsing is built as a lossless `rowan` CST pipeline (lexer → event-based
  parser → CST) with reconstruction for round-trip checks (`src/parser/*`,
  `src/syntax.rs`).
- Formatter and linter operate on file discovery over `.R` paths, then parse and
  check paths (`src/formatter/*`, `src/linter/*`, `src/file_discovery.rs`).
- Incremental parsing uses `salsa` to cache file text and parse outputs
  (`src/incremental.rs`).
- Project metadata and intended scope are defined in `Cargo.toml`: a Rust
  CLI/tooling project for R language support (LSP, formatter, linter).
- Agreed parser architecture for implementation:
  - lossless `rowan` CST as the core syntax representation
  - hand-written recursive descent + Pratt parser (expressions)
  - event-based parsing pipeline lowered into rowan
  - `salsa` used for file/parse caching first, then expanded for dependency
    graph modeling
- Delivery order is parser-first architecture, then formatter as first consumer,
  then linter/LSP features.
- CI establishes the intended engineering surface before full implementation
  lands:
  - `.github/workflows/build-and-test.yml`: cross-platform build/test, plus
    `cargo-audit` and `cargo-deny`.
  - `.github/workflows/lint.yml`: Rust lint/format checks, plus CLI-based
    formatting/lint checks over `docs/` via `cargo run -- format --check docs/`
    and `cargo run -- lint --check docs/`.
  - `.github/workflows/coverage.yml`: coverage generation with `cargo llvm-cov`.

## Key conventions in this codebase

- Treat CI commands as source of truth for quality gates; local commands should
  mirror workflow steps.
- Clippy warnings are escalated to errors (`-D warnings`) in both task config
  and CI behavior.
- Rust formatting uses `cargo fmt` with check mode in CI; keep changes
  rustfmt-clean.
- Logging-based test debugging is expected via `RUST_LOG` levels
  (normal/debug/trace/off) rather than ad-hoc test harness changes.
- Security/dependency checks (`cargo-audit`, `cargo-deny`) are part of standard
  CI, so dependency changes should remain compatible with both.
- Parser work prioritizes stable, recoverable CST shape and losslessness over
  early semantic precision.
- `SyntaxKind` variants follow rowan-style `SCREAMING_SNAKE_CASE`.
- `ravel lint` currently requires `--check`; `ravel format` only accepts
  multiple paths with `--check`.

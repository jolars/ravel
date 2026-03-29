# Copilot instructions for `ravel`

## Build, test, and lint commands

- Build (dev): `cargo build --verbose`
- Build (release): `cargo build --release`
- Run tests: `cargo test`
- Run one test by name: `cargo test <test_name_substring>`
- Run parser snapshot tests only: `cargo test --test parser_snapshots`
- Run tests with logs:
  - Debug: `RUST_LOG=debug cargo test`
  - Parser trace: `RUST_LOG=panache::parser=trace cargo test`
  - Quiet logs: `RUST_LOG=off cargo test`
- Review/accept `insta` snapshots:
  - `cargo insta review`
  - `cargo insta accept`
- Lint: `cargo clippy --all-targets --all-features -- -D warnings`
- Format check: `cargo fmt -- --check`

Taskfile equivalents:

- `task lint`
- `task format`
- `task test`
- `task test-debug`
- `task test-trace`
- `task test-quiet`

## High-level architecture

- This repository is currently in an early scaffold state: runtime code is only `src/main.rs` with a placeholder `main()` implementation.
- Project metadata and intended scope are defined in `Cargo.toml`: a Rust CLI/tooling project for R language support (LSP, formatter, linter).
- Agreed parser architecture for implementation:
  - lossless `rowan` CST as the core syntax representation
  - hand-written recursive descent + Pratt parser (expressions)
  - event-based parsing pipeline lowered into rowan
  - `salsa` used for file/parse caching first, then expanded for dependency graph modeling
- Delivery order is parser-first architecture, then formatter as first consumer, then linter/LSP features.
- CI establishes the intended engineering surface before full implementation lands:
  - `.github/workflows/build-and-test.yml`: cross-platform build/test, plus `cargo-audit` and `cargo-deny`.
  - `.github/workflows/lint.yml`: Rust lint/format checks, plus CLI-based formatting/lint checks over `docs/` via `cargo run -- format --check docs/` and `cargo run -- lint --check docs/`.
  - `.github/workflows/coverage.yml`: coverage generation with `cargo llvm-cov`.

## Key conventions in this codebase

- Treat CI commands as source of truth for quality gates; local commands should mirror workflow steps.
- Clippy warnings are escalated to errors (`-D warnings`) in both task config and CI behavior.
- Rust formatting uses `cargo fmt` with check mode in CI; keep changes rustfmt-clean.
- Logging-based test debugging is expected via `RUST_LOG` levels (normal/debug/trace/off) rather than ad-hoc test harness changes.
- Security/dependency checks (`cargo-audit`, `cargo-deny`) are part of standard CI, so dependency changes should remain compatible with both.
- For parser work, optimize for stable, recoverable CST shape and losslessness over early semantic precision.
- `SyntaxKind` variants follow rowan-style `SCREAMING_SNAKE_CASE`.

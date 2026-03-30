# Ravel roadmap

## Goal

Build a robust Rust-based foundation for R tooling with this implementation
order.

- [x] Parser/CST foundation (initial bootstrap completed; continue expanding)
- [x] Formatter (first consumer)
- [ ] Linter and language server integration (later phases)

## Architecture decisions

- [x] Use a **lossless CST** built with `rowan` (preserve all tokens and
      trivia).
- [x] Use a **hand-written parser**:
      - [x] recursive descent for structural forms
- [x] Pratt parser for expressions and operator precedence
- [x] Use an **event-based parser pipeline** (`start node` / `token` /
      `finish node`) and then lower into rowan.
- [ ] Keep semantics **static** (no R code evaluation).
- [x] Use `salsa` for file text and parse caching first; expand to dependency
      graph tracking in later phases.

## Phased plan

## Phase 0: Parser foundations

- [x] Define initial token kinds and syntax kinds (expand for full R operator
      surface in next iterations).
- [x] Implement a lossless lexer:
      - [x] preserve whitespace/newlines
      - [x] preserve comments
      - [x] lex `%...%` operators as single tokens
      - [x] distinguish `[[` and `]]` cleanly
- [x] Build initial parser infrastructure:
      - [x] token source (minimal, lexer-backed)
      - [ ] event sink
      - [ ] marker/checkpoint utilities
      - [x] parser diagnostics container (initial assignment error coverage)

## Phase 1: Expression parsing

- [x] Implement Pratt parser skeleton with explicit binding powers and
      associativity (`+`, `*`, `^`).
- [x] Cover infix precedence and parenthesized expression baseline.
- [x] Handle right-associative power (`^`) and assignment integration
      (`a <- 1 + 2`).
- [x] Add focused parser tests per operator group, including malformed infix
      cases (`1 +`, `* 2`).

## Phase 2: Structural forms and statements

- [x] Parse control and structural constructs (`if`, `for`, `while`, `function`,
      blocks).
- [x] Define statement boundary rules, especially newline-sensitive cases.
- [ ] Handle ambiguous contexts such as `=` in argument lists vs assignment.
- [x] Add recovery rules that keep CST shape stable after syntax errors.

## Phase 3: Rowan CST + validation

- [x] Build direct rowan CST construction and expose debug-tree output.

- [x] Guarantee losslessness by round-trip checks (source -> CST -> source
      text).

- [x] Add snapshot-style CST tests for initial fixture corpus (expand to broader
      representative/malformed set next).
- [x] Expand fixture corpus for lexer coverage (comments, strings, floats,
      `%...%`, `[[`/`]]`) with snapshots and losslessness checks.
- [x] Snapshot parser diagnostics per fixture, including malformed input
      (`assignment_missing_rhs`).

## Phase 3.2: Typed AST wrappers over CST (rowan)

- [ ] Introduce typed AstWrappers using rowan's built-in AST support (`AstNode`,
      `ast::support`).
- [ ] Add wrapper coverage for current core nodes (`AssignmentExpr`, `BinaryExpr`,
      `IfExpr`, `ForExpr`, `WhileExpr`, `FunctionExpr`, `BlockExpr`).
- [ ] Keep wrappers zero-cost over lossless CST (no semantic evaluation, no data
      duplication).
- [ ] Add tests validating wrapper casting/traversal against snapshot fixtures.

## Phase 3.5: CLI bootstrap

- [x] Expose parse CLI surface (`ravel parse [file] [--quiet] [--verify]`).

- [x] Support parsing from file path or stdin.

- [x] Wire `--verify` to parser losslessness invariant.

## Phase 4: Incremental and project model (`salsa`)

- [x] Model file text, token stream, parse events, and CST as salsa queries.
- [x] Implement targeted invalidation for file edits.
- [ ] Add parse performance and incremental-reparse benchmarks.

## Phase 5: Formatter v1 (first consumer)

- [x] Implement formatter rules over CST while preserving comments and
      semantics.
- [x] Add stable formatting tests (idempotence and regression suites).
- [x] Expose formatter CLI surface (`format`, `--check`).

## Phase 5.5: Project configuration (TOML, Ruff-inspired)

- [ ] Define `ravel.toml` configuration schema and defaults (human-friendly,
      explicit, and forward-compatible).
- [ ] Support configuration discovery hierarchy (cwd -> parent dirs) and
      precedence with CLI flags.
- [ ] Add sections for formatter and linter settings (start minimal, expandable).
- [ ] Validate and report configuration errors with clear file/field context.
- [ ] Add tests for config parsing, discovery, precedence, and invalid files.

## Phase 6: Linter and LSP integration (deferred)

- [ ] Add semantic layers: symbols, scopes, and lightweight inference.
- [ ] Build diagnostics and lint passes on CST + semantic model.
- [ ] Integrate with `tower-lsp-server` for IDE features.

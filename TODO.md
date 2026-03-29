# Ravel roadmap

## Goal

Build a robust Rust-based foundation for R tooling with this implementation
order.

- [x] Parser/CST foundation (initial bootstrap completed; continue expanding)
- [ ] Formatter (first consumer)
- [ ] Linter and language server integration (later phases)

## Architecture decisions

- [x] Use a **lossless CST** built with `rowan` (preserve all tokens and
      trivia).
- [ ] Use a **hand-written parser**:
      - [ ] recursive descent for structural forms
      - [ ] Pratt parser for expressions and operator precedence
- [ ] Use an **event-based parser pipeline** (`start node` / `token` /
      `finish node`) and then lower into rowan.
- [ ] Keep semantics **static** (no R code evaluation).
- [ ] Use `salsa` for file text and parse caching first; expand to dependency
      graph tracking in later phases.

## Phased plan

## Phase 0: Parser foundations

- [x] Define initial token kinds and syntax kinds (expand for full R operator
      surface in next iterations).
- [ ] Implement a lossless lexer:
      - [x] preserve whitespace/newlines
      - [ ] preserve comments
      - [ ] lex `%...%` operators as single tokens
      - [ ] distinguish `[[` and `]]` cleanly
- [x] Build initial parser infrastructure:
      - [x] token source (minimal, lexer-backed)
      - [ ] event sink
      - [ ] marker/checkpoint utilities
      - [ ] parser diagnostics container

## Phase 1: Expression parsing

- [ ] Implement Pratt parser with explicit binding powers and associativity.
- [ ] Cover prefix/infix/postfix patterns used in R expressions.
- [ ] Handle right-associative assignment chains and custom infix operators.
- [ ] Add focused parser tests per operator group.

## Phase 2: Structural forms and statements

- [ ] Parse control and structural constructs (`if`, `for`, `while`, `function`,
      blocks).
- [ ] Define statement boundary rules, especially newline-sensitive cases.
- [ ] Handle ambiguous contexts such as `=` in argument lists vs assignment.
- [ ] Add recovery rules that keep CST shape stable after syntax errors.

## Phase 3: Rowan CST + validation

- [x] Build direct rowan CST construction and expose debug-tree output.

- [x] Guarantee losslessness by round-trip checks (source -> CST -> source
      text).

- [x] Add snapshot-style CST tests for initial fixture corpus (expand to broader
      representative/malformed set next).

## Phase 3.5: CLI bootstrap

- [x] Expose parse CLI surface (`ravel parse [file] [--quiet] [--verify]`).

- [x] Support parsing from file path or stdin.

- [x] Wire `--verify` to parser losslessness invariant.

## Phase 4: Incremental and project model (`salsa`)

- [ ] Model file text, token stream, parse events, and CST as salsa queries.
- [ ] Implement targeted invalidation for file edits.
- [ ] Add parse performance and incremental-reparse benchmarks.

## Phase 5: Formatter v1 (first consumer)

- [ ] Implement formatter rules over CST while preserving comments and
      semantics.
- [ ] Add stable formatting tests (idempotence and regression suites).
- [ ] Expose formatter CLI surface (`format`, `--check`).

## Phase 6: Linter and LSP integration (deferred)

- [ ] Add semantic layers: symbols, scopes, and lightweight inference.
- [ ] Build diagnostics and lint passes on CST + semantic model.
- [ ] Integrate with `tower-lsp-server` for IDE features.

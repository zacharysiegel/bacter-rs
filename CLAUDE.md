# CLAUDE.md

Guidance for Claude Code when working in this repository.

## Project

- Rust rewrite of Bacter.
- Reference implementation: `/Users/singularity/bacter`, branch `master`; read-only.
- Models: `/Users/singularity/eafora` (workspace shape, Leptos web, wgpu renderer in `shared`, `docs/conventions/`, `scripts/`) and `/Users/singularity/singularity` (actix-web and actix-ws server patterns).
- Everything runs on this machine. Never require a hosted server (production, staging, shared) to develop or verify a change.

## Documentation

- Architecture: `docs/architecture/`, indexed by `docs/architecture/README.md`. Binding; plans elaborate it and do not reopen its decisions.
- Conventions: `docs/conventions/`, indexed by `docs/conventions/README.md`, including `git-workflow.md` and `testing.md`. Read the relevant doc before writing code in that area.
- Plans: `docs/plans/`.

## Determinism contract

- The simulation in `shared` is bit-identical on wasm32 and native.
- Integer state only; no `f32`/`f64` stored in `GameState`.
- Floats only in pure functions with integer inputs and outputs, using only IEEE-exact operations; no `mul_add`.
- Logarithm only through `libm::log`, only while building the growth chance tables.
- Randomness only from the hand-written `Pcg32`, in the defined draw order.
- No `HashMap` or `HashSet`; no `usize` in `GameState` or any wire type; iteration in the defined orders.
- Enforced by `shared/tests/forbidden_operations.rs`.
- Every simulation change or wire-type change keeps the replay fixtures passing, or regenerates the fixtures and their checksums deliberately.

## Wire types

- Only the `...Serial*` types in `shared/src/protocol/` derive bitcode; no other module or crate names bitcode.
- Changing a wire type bumps `PROTOCOL_VERSION`.

## Scripts

- Invoke the wrappers in `scripts/`, never the raw tool they wrap, including in commands written for the owner. Check for a wrapper before writing a raw tool command.

## Code rules

- `mod.rs` holds only `pub mod x;` + `pub use x::*;` pairs and doc comments.
- File order: imports, consts and statics, types each followed by its `impl`, free functions.
- `RwLock` and `Mutex` poison errors are unwrapped (`.unwrap()` or `.expect()`), never handled: a poisoned lock means a thread panicked.
- Input handler naming: trait functions (`ClickHandler::click`) use handle semantics, where the implementor checks whether the event applies; callback fields and delegating functions (`on_click`) use on semantics, where the caller has already established that the event is relevant.
- New dependencies are discussed with the owner first.

## Owner preferences

- `~/.claude/CLAUDE.md` holds the owner's preferences for code style, commits, tool use and process. Read it.

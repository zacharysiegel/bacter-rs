# Coding conventions

Per-topic convention specs that govern the bacter-rs codebase. Read the relevant doc before writing new code in that area.

These docs are the source of truth. Plans and memory files (in `~/.claude/projects/-Users-singularity-bacter-rs/memory/`) reference them rather than restating rules.

## Index

- [`types.md`](types.md): type naming (models and their `SerialIn`/`SerialOut`/`Serial` wire types, conversion placement, `Kind` enum suffix, `code()`/`as_str()`, `TryFrom<&str>` parsing).
- [`logging.md`](logging.md): log message format: `<message>; [key=value ...]` for messages with structured data; prose-only messages stand alone.
- [`shading.md`](shading.md): WGSL and shader naming: transform matrices named `<source_space>_to_<destination_space>` (`object_to_world`, `view_to_clip`), never by pipeline role alone (`model_matrix`) or acronym (`mvp`).
- [`conditional-compilation.md`](conditional-compilation.md): gate a block of target- or feature-specific items in one `#[cfg]`-ed submodule (`mod native`/`mod wasm`, `mod hydrate`/`mod ssr`) with the `#[cfg]` on the module and re-exports, not per item; one-line WHY on each gate.
- [`git-workflow.md`](git-workflow.md): default branch, stacked branches and marker commits, commits, pull requests, integration by local rebase, and the script implementing each rule.
- [`testing.md`](testing.md): what is written test-first, what is exempt, and the test defaults.

## When to add a doc here

Add a convention doc when a rule:

1. Affects how multiple features get written (not a one-off architectural choice).
2. Has been called out in code review more than once.
3. Has tradeoffs worth recording so future-you doesn't re-derive them.

Don't add a doc just to centralize trivia that's already obvious from the code.

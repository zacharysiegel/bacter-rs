# Testing conventions

What is written test-first, what is exempt, and the defaults every test follows. The tests themselves are listed in `docs/architecture/testing.md`.

## Test-first

Written test-first (a failing test, then the implementation which makes it pass):

- simulation rules;
- determinism: checksums, replay fixtures, forbidden operations, chance tables, `Pcg32`;
- the protocol codec and validation, including every `TryFrom` rejection;
- server admission and cursor clamp logic.

## Exempt

- UI views (Leptos components) and shaders. Views are verified manually; shaders by the ignored pipeline-compile test.

## Defaults

Singularity parity unless a divergence is recorded below.

- Unit tests sit inline at the bottom of the file under test, in `#[cfg(test)] mod tests { use super::*; ... }`.
- Integration tests sit in `<crate>/tests/`. A helper used by two or more test files lives in `tests/helpers/<concern>.rs`; a helper with one consumer stays a private function in that test file.
- Test names: `<function_under_test>_<scenario>`, for example `from_parts_rejects_even_increment`.
- No messages in assertions.
- A test which needs a GPU or a long run is `#[ignore]` and runs through a script in `scripts/test/`; browser and wasmtime runs also go through `scripts/test/`.
- `#[wasm_bindgen_test]` only for behaviour which exists only in a browser. Target-agnostic logic is covered by the host `#[test]` plus `./scripts/test/check-wasm.sh`.
- A simulation change or a wire-type change keeps the replay fixtures passing, or regenerates the fixtures and their checksums deliberately.

## Where bacter-rs diverges from Singularity

- The inline unit-test module is named `tests`, not `test`.

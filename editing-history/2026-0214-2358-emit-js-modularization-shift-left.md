# 2026-02-14 23:58 emit_js modularization + shift-left contracts

## Scope

This round focuses on two tracks:

1. Continue low-risk modularization of `src/codegen/emit_js.rs` without changing runtime semantics.
2. Introduce AI-oriented "shift-left" validation contracts for expected warning/error paths.

## What changed

### Rust codegen modularization

- Removed duplicate legacy module file:
  - `src/calcit/struct.rs`
- Split helpers from `emit_js.rs` into cohesive submodules:
  - `src/codegen/emit_js/tags.rs`
  - `src/codegen/emit_js/symbols.rs`
  - `src/codegen/emit_js/paths.rs`
  - `src/codegen/emit_js/runtime.rs`
  - `src/codegen/emit_js/args.rs`
  - `src/codegen/emit_js/deps.rs`
  - `src/codegen/emit_js/helpers.rs`
- Kept behavior stable by only moving logic and wiring imports/call sites.
- Added targeted unit tests in extracted modules.

### Runtime helper boundary cleanup (TS)

- Grouped runtime helpers into dedicated modules:
  - `ts-src/js-arity-helpers.mts` (`_args_throw`, `_args_fewer_throw`, `_args_between_throw`)
  - `ts-src/js-tag-helpers.mts` (`init_tags` + internal cache)
- Switched `ts-src/calcit.procs.mts` to re-export these modules, keeping public API names unchanged.

### Test/validation workflow improvements

- Added contract-based shift-left script:
  - `scripts/check-shift-left.mjs`
- Added npm script:
  - `check-shift-left` in `package.json`
- This script enforces both:
  - positive baseline must pass (`calcit/test.cirru -1`)
  - selected negative cases must fail with expected diagnostic tokens.

### Planning/documentation updates

- Expanded and updated roadmap status:
  - `drafts/project-modernization-roadmap.md`
- Added AI-oriented long-term section for:
  - immutable core constraints
  - earlier error detection
  - contractized diagnostics
  - staged execution strategy.

## Verification

- Ran targeted unit tests for extracted modules (`tags/symbols/paths/runtime/args/deps/helpers` where applicable).
- Repeated full validation with `yarn check-all` after each refactor batch.
- Ran `yarn check-shift-left` to validate diagnostic contracts.

## Notes

- Some standalone `calcit/test-*.cirru` files are intentionally warning/error reproductions or independent package entries; they are not safe to inline into `test.cirru` module list directly.
- Using a separate shift-left contract script avoids namespace collisions and preserves intent.

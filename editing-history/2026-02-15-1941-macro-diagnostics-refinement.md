# Macro Diagnostics Refinement (2026-02-15 19:41)

## Summary

This change improves diagnostic quality for macro misuse and warning flows without changing macro semantics.

## Key updates

- Added a shared diagnostics helper module:
  - `src/diagnostics_help.rs`
  - Centralizes message-to-help inference logic.
- Improved stack-rendered failure guidance:
  - `src/call_stack.rs`
  - Uses macro-context-aware help text.
  - Keeps examples targeting user-facing macros over internal helper macros.
- Improved warning-path guidance:
  - `src/bin/cr.rs`
  - Reuses shared help inference for preprocessing/codegen warning blocks.
- Improved macro argument binding diagnostics in runtime:
  - `src/runner.rs`
  - Replaces low-level `Idx(...)` style arity failures with readable signatures and missing argument names.
- Refined macro-side error wording:
  - `src/cirru/calcit-core.cirru`
  - Better `list-match` branch-shape messages.
- Kept macro signatures intact where requested:
  - `when`, `when-not`, `when-let` remain with explicit parameter forms.

## Validation

- Spot checks executed with `cr eval` for macro misuse cases:
  - `when`, `when-not`, `when-let`, `if-not`, `tag-match`, `record-match`, `list-match`, `cond`, `let`, `thread-first`, `thread-last`.
- Confirmed help text differentiates macro misuse vs proc/function arity warnings.
- Full regression passed:
  - `yarn check-all` => `EXIT:0`.

## Notes

- This iteration focuses on user-facing diagnostics quality.
- Behavior and compatibility are preserved; changes are primarily in error/help reporting paths.

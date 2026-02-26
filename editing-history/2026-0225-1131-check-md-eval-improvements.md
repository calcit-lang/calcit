# check-md and eval improvements

- Added `docs check-md --dep` (repeatable) and forwarded deps to internal eval/check-only.
- Added ns-first snippet handling in eval: merge `ns ...` tail nodes into `ns app.main`.
- Added check-md hint when `cirru` blocks are fewer than `cirru.no-check` blocks.

## 2026-02-26 follow-up

- Refactored `docs check-md` to run `cirru` / `cirru.no-run` / `cirru.no-check` in-process instead of spawning `cr` subprocesses for each block.
- Added shared dependency/core loading cache in `check-md` so modules and core snapshot load once per markdown file, then reuse per block.
- Kept warning/error details visible in failed block output while preserving pass/fail summary behavior.
- Simplified path display in logs and header output for default modules: replaced absolute module root with `<mods>/...` placeholder.
- Shortened displayed `entry` path in `check-md` header by preferring current-directory relative paths when available.

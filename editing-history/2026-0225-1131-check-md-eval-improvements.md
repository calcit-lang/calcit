# check-md and eval improvements

- Added `docs check-md --dep` (repeatable) and forwarded deps to internal eval/check-only.
- Added ns-first snippet handling in eval: merge `ns ...` tail nodes into `ns app.main`.
- Added check-md hint when `cirru` blocks are fewer than `cirru.no-check` blocks.

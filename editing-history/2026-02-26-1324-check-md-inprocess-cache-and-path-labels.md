# check-md in-process cache and path labels

- Replaced process-per-block `docs check-md` execution with in-process Rust checks for `cirru`, `cirru.no-run`, and `cirru.no-check`.
- Added shared cache for deps/core loading in `check-md`, reducing repeated module/core startup per markdown block.
- Kept warning/error details visible in block-level failure output.
- Replaced default module absolute path displays with `<mods>/...` in `loading:` logs and `check-md` deps preview.
- Normalized `check-md` entry path display to prefer current-directory relative path.

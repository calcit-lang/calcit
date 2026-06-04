# 2026-06-04 compiled executable runtime backfill

- `resolve_runtime_or_compiled_def` materialized compiled executable defs without seeding runtime, so repeated symbol lookup rebuilt fresh `Fn` values.
- Fresh `Fn` ids broke caches keyed by functions, reproducing in `memof` where `memof1-call` and `memof1-call-by` missed cache hits under `cr calcit.cirru`.
- `materialize_compiled_executable_payload` now writes the resolved executable back into runtime-ready storage, preserving stable function identity across lookups.
- Program tests now assert runtime backfill for compiled executable resolution and `memof` plain `cr` execution passes again with the patched binary.
# Profiling tools reorganization

## Summary

- Moved profiling scripts from `scripts/` into dedicated `profiling/` directory.
- Added `profiling/README.md` with end-to-end usage for xctrace and samply workflows.
- Removed obsolete `scripts/profiling.sh` flamegraph helper script.

## Knowledge notes

- Keep profiling tooling isolated under `profiling/` to reduce script namespace clutter.
- `samply` profiling should run against built `target/*/cr` binaries to avoid sampling `rustc` compile threads.
- Preserve `.tmp-profiles/` as transient output directory for trace artifacts.

# Runtime boundary lookup cleanup

## Summary

- Consolidated runtime-vs-compiled lookup helpers inside `program` and removed extra `runner` bridging paths.
- Tightened snapshot fallback and codegen metadata lookup so source-backed defs no longer silently rely on runtime-derived compiled entries.
- Skipped runtime-only placeholder defs in JS/IR codegen and added program-level regression tests for reload invalidation and snapshot behavior.
- Reframed the runtime-boundary draft into a closure plan focused on stabilizing compiled/runtime boundaries, adding watch-reload regression coverage, and avoiding new architectural layers.
- Simplified `runner` and `preprocess` lookup flow by removing thin wrappers, deduplicating repeated symbol fallback order, and collapsing silent program-value reads onto shared helpers.
- Added regression coverage for reload package clearing, source-backed snapshot rebuild after changes, compiled fallback cache behavior, and strict-vs-lenient runtime resolution semantics.

## Knowledge points

- Runtime execution helpers should live in `program` so `runner` only maps runtime state to evaluation flow and user-facing errors.
- Metadata queries such as codegen type hints should prefer compiled/source schema and only fall back to ready runtime values, never by executing compiled payloads.
- Reload invalidation needs direct transitive-dependency tests plus namespace-header coverage; otherwise runtime cache cleanup regresses quietly.
- If compiled execution is used as a fallback read path, tests must assert it does not implicitly backfill runtime cells; otherwise compiled/runtime boundaries drift back together.
- Keep cleanup work biased toward removing duplicate lookup order and unused parameters before introducing any new abstraction; the stable payoff is clearer boundaries with lower entity count.
- `runner`-side error mapping for strict runtime resolution is still a meaningful boundary, but repeated namespace lookup order and required-value program reads can be shared safely.

## Validation

- `cargo fmt`
- release fibo profiling during optimization review
- `cargo test clear_runtime_caches_for_reload -- --nocapture`
- `cargo test snapshot_rebuilds_changed_source_backed_def_after_reload_changes -- --nocapture`
- `cargo test program::tests -- --nocapture`
- `cargo test -q`

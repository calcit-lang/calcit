# Runtime boundary lookup cleanup

## Summary

- Consolidated runtime-vs-compiled lookup helpers inside `program` and removed extra `runner` bridging paths.
- Tightened snapshot fallback and codegen metadata lookup so source-backed defs no longer silently rely on runtime-derived compiled entries.
- Skipped runtime-only placeholder defs in JS/IR codegen and added program-level regression tests for reload invalidation and snapshot behavior.

## Knowledge points

- Runtime execution helpers should live in `program` so `runner` only maps runtime state to evaluation flow and user-facing errors.
- Metadata queries such as codegen type hints should prefer compiled/source schema and only fall back to ready runtime values, never by executing compiled payloads.
- Reload invalidation needs direct transitive-dependency tests plus namespace-header coverage; otherwise runtime cache cleanup regresses quietly.

## Validation

- `cargo fmt`
- release fibo profiling during optimization review
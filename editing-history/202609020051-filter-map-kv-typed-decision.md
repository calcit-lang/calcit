# 2026-09-02 00:51 — typed `filter-map-kv` decision

## Summary

- Added generic nominal enum `MapEntryDecision<K, V>` with `:keep K V` and
  `:drop` variants.
- Added `filter-map-kv` and `.filter-map-kv`, with a schema that propagates the
  callback decision payload types to the output `Map` key/value types.
- Kept legacy `map-kv` sentinel behavior for compatibility, while documenting
  it as a migration-only path rather than a supported filtering contract.
- Added native, JavaScript, static-inference, and WASM coverage.

## Knowledge captured

- Returning `nil` or an arbitrary enum from `map-kv` was implemented by the
  shared Calcit core runtime path, but the WASM `map-kv` lowering only understood
  a two-item pair. The old filtering behavior was therefore cross-backend
  inconsistent.
- A generic nominal decision enum lets preprocessing infer
  `MapEntryDecision<R, S>` and consequently `Map<R, S>` without using a
  heterogeneous pair-list type or a `nil` union.
- WASM does not automatically lower every higher-order core helper. Public HOFs
  that accept inline lambdas need explicit import/symbol/function dispatch and a
  dedicated loop emitter. Named enum values use the memory layout
  `[payload_count, tag, payload...]`; `filter-map-kv` reads `:keep` payloads at
  offsets 16 and 24 and leaves the accumulator unchanged for `:drop`.
- `scripts/test-wasm.sh` prefers an existing release binary. During development,
  build `cr-wasm` and set `CR_WASM_BIN=./target/debug/cr-wasm` so a regression
  run exercises the current source rather than a stale release artifact.

## Follow-up

- Migrate known `map-kv` sentinel call sites to `filter-map-kv`.
- After the replacement API is available in a release, add a stable strict-mode
  diagnostic for legacy `nil`/anonymous-enum callback sentinels and eventually
  make `map-kv` transform-only across all backends.

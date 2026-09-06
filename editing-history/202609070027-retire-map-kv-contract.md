# Retire the unproven map-kv contract

## Context

Milestone issue #827 tracks the legacy `map-kv` callback contract. The runtime
accepts either a two-item list or historical nil/enum drop sentinels, so the old
`Map<K,V> -> Map<R,S>` schema claimed output types that were unrelated to the
callback result. This change also carries the two valid CodeRabbit findings
that arrived after PR #892 had already merged.

## Changes

- Marked the legacy `map-kv` result as an explicit `Dynamic` compatibility
  boundary. Compatibility mode emits `W_MAP_KV_UNPROVEN_CONTRACT`; strict mode
  emits `E_MAP_KV_UNPROVEN_CONTRACT` and directs callers to `filter-map-kv` with
  `MapEntryDecision`.
- Migrated bundled `tagging-edn` to the typed API and documented
  `filter-map-kv` as the sole recommended typed entry transformation API.
- Classified and locked the single new reviewed core Dynamic position. This is
  an intentional replacement of false generic evidence, not unrelated Dynamic
  growth.
- Added native and generated-JavaScript coverage for callback count, empty
  input, duplicate output keys, and callback exception propagation.
- Replaced display-EDN comparison of nested struct fields and enum payloads
  with recursive data-schema comparison that preserves qualified nominal
  identity while continuing to exclude attached impls.
- Added a TypeRef arity-resolution reentrancy guard so self-referential enum
  schemas remain finite while ordinary outer applications still perform arity
  validation.

## Verification

- `cargo test --workspace --all-features` (729 library + 306 native binary +
  23 WASM binary tests)
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `fnm exec --using=24.4.1 yarn check-all` (native, generated JS, IR, WASM,
  agent-interface, literal-path, and quality gates)
- focused legacy map-kv, nested nominal identity, and self-referential enum
  regressions

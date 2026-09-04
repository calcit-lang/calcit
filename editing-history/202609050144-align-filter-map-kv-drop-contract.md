# Align filter-map-kv drop contracts

## Context

`filter-map-kv` already models its callback with the nominal
`MapEntryDecision<K, V>` enum, but the Calcit implementation wrapped the
unchanged accumulator in a redundant `identity` call. The shared WASM emitter
also described both callback results as pair lists even though the filtering
variant consumes the enum layout.

## Change

- Return the accumulator directly from the `:drop` branch.
- Document the pair-list and `MapEntryDecision` layouts separately at the
  shared WASM lowering boundary.

Existing core and WASM regressions exercise both transformed `:keep` entries
and omitted `:drop` entries; no runtime behavior or schema changes are intended.

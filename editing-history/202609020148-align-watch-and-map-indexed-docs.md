# Align watcher contract and collection documentation

## Context

PR review identified that `add-watch` still accepted an unconstrained generic
key even though every runtime path requires a Tag. The initial `map-indexed`
contract cleanup also left its documentation describing a generic collection
while the schema and implementation are list-specific.

## Changes

- Narrow the `add-watch` key from generic `K` to `Tag` and remove the unused
  key generic.
- Document the watcher callback order as `(new-value old-value)` and both Unit
  returns.
- Document `map-indexed` explicitly as `List<T> -> List<U>` with an
  `(index value)` callback.

The quality totals remain at 297 unresolved schema-Dynamic positions; this is
a soundness and documentation correction rather than a Dynamic-count change.

## Validation

- Queried both definitions after Snapshot edits.
- Bundled-core quality baseline passes unchanged.
- Full repository gates run after the review fix.

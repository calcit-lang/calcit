# Close Dynamic output-propagation evidence

## Context

The bundled-core inventory still described two caller-visible output positions
as wholly unresolved even though preprocessing already recovers them from
bounded static evidence. That obscured the remaining migration boundary and
made the release audit treat proven output flow like an open receiver contract.

## Change

- Classify only `get-in`'s Option payload and `filter`'s result as
  compiler-specialized contracts.
- Add focused regressions for literal nested-Map payload inference and the
  List/Map/Set filter dispatch matrix.
- Keep Dynamic receivers, dynamic paths, Struct traversal, and unsupported
  collection capabilities in the migration queue.
- Assign every remaining migrate row to calcit#701 under the 0.14.0 milestone;
  optional indexed-access slot correctness remains the narrower calcit#694.

The schema-Dynamic inventory and quality budgets do not change. The release
consumer proof uses `calcit-lang/respo-calcit-workflow` commit
`de5d2311bc19bd25b862ccb6504bc5125f1bf739`: `query type-at` resolves the public
`respo.cursor/update-states` call in `app.updater/updater` to
`app.types/Store` with exact confidence and no diagnostics.

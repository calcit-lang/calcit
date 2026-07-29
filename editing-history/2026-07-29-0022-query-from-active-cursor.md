# Query from the active cursor

- Allowed definition-oriented queries to use `@cursor` as their target.
- Added cursor-scoped structural expression search with absolute result paths.
- Made `query type-at @cursor --path @cursor` resolve one validated cursor state.
- Rejected conflicting explicit filters for cursor-scoped searches.

Real-project checks covered search selection, type evidence, and bounded semantic context.

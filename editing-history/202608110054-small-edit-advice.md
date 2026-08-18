## Suppress structural-edit advice for small definitions

- Structural-edit guidance now requires either the existing or incoming Cirru
  tree to contain at least 32 AST nodes.
- Tiny definitions do not benefit from a path-finding workflow, so full
  definition updates no longer emit a misleadingly heavyweight warning.
- Keep the threshold in `analyze_cirru_edit_advice` so every caller shares the
  same behavior, including identical-definition advice.
- `&list:first` now consistently reports its nullable `Optional<T>` result to
  static analysis, matching the Core schema and its `nil` result for empty
  lists.
- Core macros and comparison helpers that first establish non-emptiness now
  use the explicitly indexed low-level read `&list:nth xs 0`.

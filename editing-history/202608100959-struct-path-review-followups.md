## Struct path-operation review follow-ups

- `contains-in?` now follows the same Struct boundary as the other collection
  path APIs: a path may not enter a declared Struct field. Direct typed field
  access keeps required-field diagnostics and return types precise.
- The preprocessor reports `W_STRUCT_PATH_OPERATION` for `contains-in?`, with
  a regression test covering all five path APIs.
- Runtime guidance for `dissoc-in` now explains that declared Struct fields
  cannot be removed; model optionality explicitly or convert to a map first.
- Module ref fetching now uses `--prune`, so stale remote-tracking branches do
  not remain selectable after an upstream branch deletion.

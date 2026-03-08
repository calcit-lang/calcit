# Remaining `:symbol` schema cleanup

## Summary

- audited the remaining `:symbol` occurrences in `src/cirru/calcit-core.cirru`
- converted leftover collection schemas that were still using `:symbol` as a generic placeholder to proper type variables
- fixed set helpers such as `&include`, `&set:to-list`, and `&union`
- fixed list helpers such as `&list:mappend`, `&list:rest`, `&list:reverse`, `&list:slice`, `&list:sort-by`, and `&list:map-pair`
- left the `symbol?` predicate unchanged because its `:symbol` usage is the real runtime type check, not a generic placeholder

## Validation

- `yarn check-all` passes after the cleanup
- grep over `src/cirru/calcit-core.cirru` now leaves only the intentional `symbol?` occurrence

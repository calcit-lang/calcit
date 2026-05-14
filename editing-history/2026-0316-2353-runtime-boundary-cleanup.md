# Runtime Boundary Cleanup

## Summary

- removed the dead `CalcitImport.coord` compatibility path and the related runner parameter flow
- renamed remaining `evaled`-era runtime lookup terminology to `runtime-ready` / `runtime cache`
- synced the runtime boundary refactor draft with the current migration state and recent cleanup progress

## Knowledge

- `CalcitImport.def_id` is now the stable import-side runtime lookup anchor; the old `coord -> EntryBook` path no longer needs to be preserved in import metadata
- type-annotation lookup registration should describe runtime-ready value lookup, not the removed evaled-store model
- incremental reload messaging should talk about clearing runtime caches rather than evaled states to match the current architecture
- this migration tail is now mostly about narrowing fallback bridges and improving reload coverage, not about preserving old runtime naming or lookup compatibility
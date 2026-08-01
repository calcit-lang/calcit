# Unify snapshot entries and run modes

- Replaced the split top-level `:configs` plus `:entries` model with a single `:entries` map. `:default` is now the required implicit entry and the project version lives at top-level `:version`.
- Added entry-level `:mode` with the supported values `:native` and `:js`. A bare `cr <snapshot> [--entry name]` now follows the selected entry's mode; the explicit `js` subcommand remains available as an override.
- Kept legacy snapshot loading compatible: old `:configs` is migrated in memory to `entries.default` with native mode, while canonical writes emit only the unified schema.
- Updated config/query/diff/type-slot/module selection, embedded core serialization, repository snapshots, bundle/sync scripts, CLI help, RFCs, and user/Agent documentation.
- Added coverage for new-format round trips, legacy migration, configured JS dispatch, and default-entry module extraction.

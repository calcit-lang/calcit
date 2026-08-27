# Retire legacy Snapshot config aliases

- Stop decoding top-level `:configs` at runtime and while embedding `calcit.core`; point legacy projects to Calcit 0.13.50 for the one-way `:entries.default` migration.
- Remove silent `calcit.cirru` to `compact.cirru` path aliasing across runtime, query, docs, and diff commands while retaining sibling detection for an actionable error.
- Remove the formatter advisory for a shape that current Calcit no longer loads, and align README, Agent guidance, and Snapshot docs with the released filename cutoff.
- Validate the strict loader against the latest `Respo/respo` and `calcit-lang/recollect` main branches: native checks/tests, Markdown examples, generated JavaScript tests, quality gates, and production Vite builds remain green.

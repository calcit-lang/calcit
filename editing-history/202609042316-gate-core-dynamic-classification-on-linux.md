# Gate core Dynamic classification on Linux

## Context

The bundled-core Dynamic classification is checked in and locally verified by
`yarn check-all`, but the GitHub pull-request and release workflows did not run
the classifier drift check. A stale inventory could therefore pass the primary
Linux release path even while its owner or migrate/retain decisions no longer
matched the compiler report.

## Change

- Run `node scripts/core-dynamic-classification.mjs --check` in the
  `ubuntu-latest` pull-request job after the core quality baseline check.
- Run the same check in the `ubuntu-latest` publish job before release build and
  publication.

This keeps the generated inventory, report revision, owners, and migration
decisions tied to the same Linux-tested compiler used by CI and releases.

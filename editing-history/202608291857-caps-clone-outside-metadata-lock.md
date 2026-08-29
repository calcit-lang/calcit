# Keep network work outside the caps metadata lock

## Context

`caps` resolves independent modules in parallel. A cache miss previously acquired the global `.metadata.lock` before cloning a repository and held the lock for the entire network operation. A clone lasting longer than the lock retry window caused otherwise independent workers to fail with a misleading lock timeout.

## Change

- Keep the first cache lookup and metadata refresh under a short lock.
- Clone a missing repository into its process-unique temporary path without holding the global lock.
- Reacquire the lock only to resolve an installation race, atomically rename the clone into the content-addressed store, and update metadata.
- Preserve validation when another process installs the same commit first.

## Verification

- Rust tests and strict Clippy pass.
- TypeScript compilation and the 17/17 agent-interface suite pass.
- A real cold-cache `caps --strict --ci` run over calcium-workflow's dependency graph cloned concurrent modules without a metadata-lock timeout.

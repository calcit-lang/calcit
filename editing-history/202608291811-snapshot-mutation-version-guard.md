# Snapshot mutation toolchain guard

## Context

Calcit 0.13.60 writes canonical symbol keys for Snapshot namespaces and definitions. A globally installed newer CLI could therefore rewrite a project whose `deps.cirru` and CI still pin an older reader, leaving the project unreadable to its declared toolchain.

## Change

- Added a shared preflight that reads the `deps.cirru` adjacent to the selected Snapshot.
- Snapshot-writing `edit`, `tree`, `config`, and cursor operations now reject an exact `:calcit-version` mismatch before loading or writing the target file.
- Read-only commands, dry-runs, cursor navigation, projects without `deps.cirru`, and legacy manifests without `:calcit-version` remain usable.
- Invalid non-string or non-semver declarations fail with a manifest-specific diagnostic.
- The failure explains both safe paths: use the pinned Calcit release, or explicitly upgrade the project with `caps upgrade --all`.

## Verification

- Unit coverage for matching, absent, mismatched, and invalid declarations.
- Real-project smoke confirmed `config show` remains readable while `edit format` is blocked without changing the Snapshot hash.
- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`
- `yarn compile`
- `yarn check-agent-interface` (17/17)
- `yarn check-all` (Calcit core 223/223, JS/IR/WASM and performance checks)

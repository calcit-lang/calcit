# Scope development dependencies in recursive resolution

## Knowledge points

- `deps.cirru` now distinguishes consumer-facing `:dependencies` from root-only
  `:dev-dependencies`.
- A root project resolves and installs both groups. A materialized dependency module exposes only
  its `:dependencies` to the recursive graph, preventing its tests, examples, documentation tools,
  and maintenance modules from leaking into consumers.
- The same repository may appear in both root groups only when both declarations use the same ref;
  conflicting refs fail before installation or before `caps add` writes the file.
- `caps add --dev` and `caps remove --dev` manage development dependencies. `caps outdated` and
  `caps upgrade --all` inspect both root groups and update the declaration in its original group.
- Upgrade guidance should ask projects to audit dependency intent, move project-only tooling into
  `:dev-dependencies`, and confirm the resulting boundary with `caps tree`.

## Validation

- `cargo fmt`
- `cargo clippy -- -D warnings`
- `yarn compile`
- `cargo test`
- `yarn check-agent-interface`
- `yarn check-all`

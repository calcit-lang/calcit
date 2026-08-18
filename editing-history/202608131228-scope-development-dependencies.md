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
- A legacy-project upgrade must be staged: record the old behavior, update `cr`/`caps`, migrate the
  dependency graph and Snapshot, then run every entry through strict preprocessing before tightening
  dynamic dispatch and static debt baselines.
- `--check-only` is a blocking preprocessing gate, including warnings, but it covers the selected
  entry's reachable path rather than every public definition. `check-types`, `weak-types`, and
  `deprecated` are report commands whose JSON summaries require explicit CI comparison.
- Type-debt baselines should compare separate categories instead of one total. For coverage, track
  `none` and `none + partial` so progress from none to partial is accepted; track dynamic, nil,
  Optional compatibility, and deprecated calls independently.

## Validation

- `cargo fmt`
- `cargo clippy -- -D warnings`
- `yarn compile`
- `cargo test`
- `yarn check-agent-interface`
- `yarn check-all`
- `cr docs format-md docs/run/upgrade.md --check`
- `cr docs check-md docs/run/upgrade.md --entry calcit/test.cirru --failures-only`
- `cr docs graph check` (with a temporary writable HOME/cache)

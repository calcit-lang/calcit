# Definition-attached core tests

- Added backward-compatible `:tests` metadata to compact, detailed, and
  embedded core snapshots. Tests have stable names, tags, and one Cirru
  expression; empty lists remain omitted from compact snapshots.
- Embedded `calcit.test` in `calcit-core.cirru` with `is`, `is=`, `is-not=`,
  `throws?`, `is-throws`, and `fail`. Ten attached tests cover success and
  rejection behavior without an external module.
- Added `cr test` discovery, namespace/definition/name/tag filters, list and
  fail-fast modes, a versioned one-line JSON report, and conservative
  `--affected ns/def` selection over transitive compiled `DefId` dependencies.
- Added named `edit add-test` / `edit rm-test` operations, `query tests`, test
  data in definition/context queries, test-body usage discovery, and test-aware
  program diffs and definition revisions.
- Redirected runtime output and diagnostics away from stdout in JSON test mode
  so coding agents can always parse one report envelope.
- Documented the metadata model, assertion API, editing workflow, test runner,
  and affected-test selection in the feature guide and Agent guide.

Local review follow-ups:

- Reject duplicate names in detailed snapshots as well as compact/binary
  snapshots.
- Cover explicit named-test overwrite and test metadata program diffs.
- Extend core assertion tests across false/error branches.

Validation:

- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test` (372 library, 2 caps, 196 cr tests passed)
- `yarn compile`
- `yarn check-agent-interface` (12/12)
- `yarn check-all` (native, JavaScript, and WASM passed)
- `cr src/cirru/calcit-core.cirru test calcit.test` (10/10)
- JSON test output parsed as one line with 10/10 results
- Globally installed `cr` exercised against Respo queries and a temporary
  snapshot copy for add/query/remove test metadata
- `git diff --check`

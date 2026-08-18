# Audit the legacy project upgrade playbook

## Findings

- An old project cannot safely treat dependency upgrade success as completion. The workflow must
  preserve an old-toolchain behavior baseline and separate tool, dependency, Snapshot, type, and
  behavior changes into reviewable stages.
- New CLI validation has distinct failure semantics. `--check-only`, examples, Markdown checks, and
  tests are blocking gates; static type/deprecation analysis emits reports that require explicit
  JSON summary comparison in CI.
- Strict preprocessing must run for every named entry, then run again with `--warn-dyn-method`.
  Entry reachability does not replace tests/examples for public library definitions.
- Gradual typing needs category-aware baselines for none/not-full coverage, dynamic locations,
  nil/Optional debt, and deprecated calls. A single aggregate can hide category regressions.
- Snapshot migration must verify which legacy file is authoritative and must not claim that Calcit
  no longer depends on a Snapshot.

## Documentation validation

- `cr docs format-md docs/run/upgrade.md --check`
- `cr docs check-md docs/run/upgrade.md --entry calcit/test.cirru --failures-only`
- `cr docs graph check` with a temporary writable HOME/cache: 22 nodes, 51 edges
- `git diff --check`

# Make dependency-boundary audits discoverable to agents

## Knowledge points

- `:dependencies`, `:dev-dependencies`, configured entry modules, and statically reachable source paths
  answer different questions. An installed module must not be classified as runtime-only from its presence
  under `.calcit/modules/`.
- Inspect root declaration intent in `deps.cirru`; use `caps tree` and `caps why <owner/repo>` to explain
  the resolved recursive graph. The current display combines both root groups, so it cannot be the
  authority for runtime versus development declaration intent.
- Use `cr config modules [--entry <name>]` to inspect each selected entry's module configuration, then
  run `cr --check-only` for default and every release/CI entry. Named entries do not inherit default
  modules.
- `docs check-md` defaults to modules from the default entry. Documentation-only dependencies require
  appropriate explicit module paths through repeatable `--dep`; these checks, like entry preprocessing,
  are static evidence and do not prove dynamic loading or external consumer use.

## Validation

- `cr docs check-md docs/run/load-deps.md --entry calcit/test.cirru --failures-only`
- `cr docs check-md docs/CalcitAgent.md --entry calcit/test.cirru --failures-only`

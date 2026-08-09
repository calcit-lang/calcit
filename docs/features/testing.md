---
title: "Definition-Attached Testing"
scope: "core"
kind: "guide"
category: "features"
aliases:
  - "calcit.test"
  - "cr test"
  - "affected tests"
  - "test metadata"
---

# Definition-Attached Testing

Calcit stores strict tests beside a definition's code, documentation, examples, schema, and tags. Tests are named metadata entries rather than ordinary definitions, so tools can discover, select, and run them without relying on naming conventions.

Examples remain executable documentation. Tests are CI guardrails: an assertion or preprocessing failure makes `cr test` exit unsuccessfully, while other selected tests continue unless `--fail-fast` is used.

## Add a Test

Import the built-in assertions where the test expression will run:

```bash
cr edit add-import app.main --code 'quote $ calcit.test :refer $ is is= is-not= is-throws throws? fail'
```

Attach a stable, named test to a definition:

```bash
cr edit add-test app.main/add adds-two-numbers \
  --tags unit,fast \
  --code 'quote $ is= 3 (add 1 2)'
```

The expression is compiled in the owning definition's namespace. It can use that namespace's imports and local definitions.

Use a file or stdin for a multiline expression. As with other AST edit commands, the input must contain exactly one quoted node.

## Inspect and Maintain Tests

```bash
cr query tests app.main/add
cr query context app.main/add --format json
cr edit add-test app.main/add adds-two-numbers --overwrite --code 'quote $ is= 5 (add 2 3)'
cr edit rm-test app.main/add adds-two-numbers
```

Test names must be non-empty and unique within one definition. Replacing one requires explicit `--overwrite`; removal uses the stable name rather than an array index.

The persisted shape is equivalent to:

```cirru.no-check
:tests $ []
  %{} 'TestEntry
    :name |adds-two-numbers
    :tags $ #{} :unit :fast
    :code $ quote $ is= 3 (add 1 2)
```

Snapshots written before this field existed load with an empty test list.

## Run Tests

```bash
# All project-owned tests
cr test

# One namespace or definition
cr test app.main
cr test app.main/add

# Exact name and tags
cr test app.main --name adds-two-numbers
cr test --tag unit --tag fast

# Inspect selection without executing
cr test app.main --list

# Emit one machine-readable JSON envelope on stdout
cr test --format json
```

Each test is compiled as its own synthetic function and executed independently. Reports use the stable identifier `namespace/definition#test-name`.

Without a scope, `cr test` discovers tests only in namespaces defined by the input snapshot. Tests bundled with `calcit-core.cirru` or loaded modules are excluded, so an external project does not accidentally run Calcit's own test suite. Pass an explicit namespace such as `cr test calcit.test` when maintaining the core assertions.

In JSON mode, runner output produced by `println`/`echo` is redirected to stderr so stdout remains one parseable report envelope.

## Run Affected Tests

```bash
cr test --affected app.math/add
cr test --affected app.math/add --affected app.math/subtract
cr test --affected app.math/add --list
```

For affected selection, Calcit preprocesses every candidate test and follows the compiled `DefId` dependency graph transitively. It always includes tests attached directly to a changed definition. If dependency analysis for a test fails, that test is conservatively selected so a static-analysis problem cannot silently hide a failing guardrail.

Normal `cr` execution does not run tests implicitly. CI and coding agents should invoke `cr test` explicitly.

## Built-in Assertions

The `calcit.test` namespace is embedded in `calcit-core.cirru` and requires no external module:

- `is` asserts a truthy expression.
- `is=` and `is-not=` compare values.
- `throws?` reports whether an expression raises.
- `is-throws` requires an expression to raise.
- `fail` raises immediately with a message.

All assertion macros evaluate each supplied expression once.

## Execution Cost

Normal `cr test` runs compile each selected test only when it is about to run.
This keeps `--fail-fast` responsive and avoids preprocessing tests after the
first failure. `--affected` intentionally compiles its candidate tests first:
it needs their static dependency graph to select a safe, transitive subset.

Tests execute the selected test and its runtime dependencies; documentation
and examples are not themselves executed as tests. Prefer a short
definition-local test for one API contract, and reserve integration fixtures
for behavior that actually crosses definitions or backends.

## Target Coverage

Definition-attached tests run in the native `cr test` runner. Keep a compact
`test-*.cirru` fixture when behavior must be compiled and executed by another
target. For example, `test-string.main/test-bitwise` is wrapped in `inside-js:`
so the full `yarn try-js` flow continues to verify JavaScript bit operations,
while its ordinary API assertions live beside the core definitions. WASM
exports and FFI checks similarly remain in the dedicated WASM fixtures.

## Core Test Placement

For a core function, macro, or builtin whose behavior can be expressed in one
namespace, attach the test directly to that definition in `calcit-core.cirru`:

```bash
cr src/cirru/calcit-core.cirru edit add-test calcit.core/range creates-half-open-range \
  --tags unit,core \
  --code 'quote $ assert= ([] 2 3 4) (range 2 5)'
```

The repository runs this suite with `yarn try-core-tests`, which explicitly
loads `calcit-core.cirru` and runs its `:unit` tests. Keep `calcit/test-*.cirru`
when a case verifies several definitions together, parser syntax, stateful
behavior, JavaScript/WASM code generation, or a full program flow. Those files
are integration fixtures rather than a substitute for definition-local tests.

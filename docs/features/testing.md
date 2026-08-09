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

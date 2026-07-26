---
title: "Querying Definitions"
summary: "使用 cr query context/defs/def/type/search/find/usages 聚合 Snapshot 元数据与静态语义"
scope: "core"
kind: "reference"
category: "run"
aliases:
  - "query defs"
  - "query ns"
  - "query def"
  - "query type"
  - "query type-at"
  - "query context"
  - "usages"
  - "find symbol"
  - "search-expr"
  - "search expr"
entry_for:
  - "cr query ns"
  - "cr query defs"
  - "cr query def"
  - "cr query type"
  - "cr query type-at"
  - "cr query context"
  - "cr query find"
  - "cr query usages"
  - "cr query search-expr"
id: core/run/query
parent: core/run
related:
  - core/run/edit-tree
  - core/features/list
requires:
  - core/agent
leads_to:
  - core/run/edit-tree
---

# Querying Definitions

Calcit provides a powerful `query` subcommand to inspect code, find definitions, and analyze usages directly from the command line.

## Core Query Commands

### List Namespaces (`ns`)

```bash
# List all loaded namespaces
cr query ns

# Show definitions in a specific namespace
cr query ns calcit.core
```

### Read Code (`def`)

```bash
# Show full source code of a definition
cr query def calcit.core/assoc

# Builtin helpers without snapshot source still return metadata
cr query def calcit.core/to-js-data
```

For source-backed definitions, `query def` prints the stored Cirru body. For special builtin helpers such as `calcit.core/to-js-data`, it falls back to builtin metadata (doc, schema, examples count) even when no snapshot source exists.

Local metadata queries (`ns <name>`, `defs`, `def`, `peek`, `examples`, `schema`, `pkg`, and `config`) first read only the main Snapshot. Modules/core are loaded only when the requested namespace is not local. This keeps repeated Agent navigation fast and avoids unrelated dependency warnings; semantic queries such as `type`, `type-at`, and `context` still load the metadata needed for static resolution.

### Peek Signature (`peek`)

```bash
# Show documentation and examples without the full body
cr query peek calcit.core/map
```

### Check Examples (`examples`)

```bash
# Extract only the examples section
cr query examples calcit.core/let

# Builtin helpers can also expose curated examples when available
cr query examples calcit.core/to-js-data
```

To execute stored examples without running an entire namespace's examples, use `cr analyze check-examples --ns app.main --def target-name`.

### Read Schema (`schema`)

```bash
# Function and value schemas use the same query
cr query schema app.main/main!
cr query schema 'app.main/*enabled?'

# Versioned machine-readable envelope with canonical schema and Cirru tree
cr query schema 'app.main/*enabled?' --json
```

Parameterized value schemas are rendered directly, for example `:: :ref :bool`; they are no longer hidden as `(none)` merely because they are not function schemas. `--json` emits actual JSON, including both the canonical one-line schema and its Cirru tree, rather than a Cirru EDN fragment labeled as JSON.

### Find Symbol (`find`)

```bash
# Search for a symbol across ALL loaded namespaces
cr query find assoc
```

### Analyze Usages (`usages`)

```bash
# Find where a specific definition is used
cr query usages app.main/main!
```

### Search Text (`search`)

```bash
# Search for raw text (leaf values) across project
cr query search hello

# Limit to one definition
cr query search hello --filter app.main/main!

# Stable paths and matched trees in one JSON envelope
cr query search hello --filter app.main/main! --format json
```

### Search Expressions (`search-expr`)

```bash
# Search structural expressions (Cirru pattern)
cr query search-expr "fn (x)"

# Limit to one definition
cr query search-expr "fn (x)" --filter app.main/main!

# `--json` decodes the pattern; `--format json` controls result encoding
cr query search-expr '["fn",["x"]]' --json --filter app.main/main! --format json
```

Search JSON results contain a summary plus definition rows with `code@...` paths and the matched Cirru tree. `--parent-path` also returns the editable parent path for leaf searches. A local `--filter` uses shallow Snapshot loading; dependency modules are loaded only if the filtered namespace is not local or an explicit `--entry` is requested.

### Inspect Static Type Methods (`type`)

```bash
# Builtin type
cr query type :number

# Parameterized type; pass Cirru directly, without an extra outer parenthesis layer
cr query type ':: :list :number'

# A definition with an explicit static schema
cr query type calcit.core/ceil

# Machine-readable result; stdout is one JSON value
cr query type :number --format json
```

`query type` loads and preprocesses static metadata but does not run the project init or reload function. It lists methods in dispatch-precedence order and shows the impl that contributes each method. Definition targets first use an explicit schema, then static source inference. This allows `defstruct` and `defenum` declarations with a dynamic entry schema to expose their named type and methods without constructing a runtime value. If neither source is sufficient, query a concrete type annotation instead.

### Inspect an Expression Type (`type-at`)

Use a Snapshot path returned by `query search`, `query context`, or another structural query:

```bash
# Inspect one field-access expression without running the project entry
cr query type-at test-record.main/sum-point --path code@3.1

# Machine-readable evidence envelope
cr query type-at test-record.main/sum-point --path code@3.1 --format json
```

`query type-at` statically preprocesses the selected definition and reports:

- the inferred type and confidence;
- the expected type supplied by a return schema, callable parameter, `if` condition, or `assert-type`;
- relevant typed bindings and their Snapshot paths;
- statically resolvable methods and implementation origins;
- evidence, diagnostics, definition revision, and follow-up commands.

The command does not invoke the project init/reload function. A dynamic FFI boundary is labeled `intentional-js-ffi` when the enclosing schema declares `:features $ #{} :js-ffi`; unresolved expressions remain explicit rather than triggering a runtime fallback. Named `defstruct`/`defenum` values are retained as source-backed type references, so field and method metadata can be resolved without constructing application values.

### Gather Definition Context (`context`)

```bash
# One bounded view for understanding or preparing to edit a definition
cr query context app.main/main!

# Return the typed result envelope as JSON
cr query context app.main/main! --format json

# Use a smaller content budget and include dependency/core usages
cr query context app.main/main! --budget 2400 --deps

# Builtin helpers without a Snapshot body use curated metadata
cr query context calcit.core/to-js-data --format json
```

`query context` combines information that otherwise requires several commands:

- definition identity and deterministic revision;
- Snapshot doc, tags, schema features, examples, and a bounded code preview;
- a JSON syntax tree for small code/example forms, or a follow-up command when omitted;
- trusted type-coverage state and static methods;
- direct dependencies and usage locations such as `code@3.2`;
- unresolved versus intentional (`:js-ffi`) dynamic-type diagnostics;
- suggested next commands for selectively expanding truncated sections.

The numeric paths are scoped to the returned revision. Re-query after a change before using a path for editing. `--budget` is an approximate character budget for variable-size content; explicit `--dependency-limit`, `--usage-limit`, and `--example-limit` bounds are also available.

With `--format json`, stdout contains one JSON envelope. Command explanations and platform registration remain on stderr, so callers should consume stdout as the machine result.

## Quick Recipes (for fast locating)

### Locate a symbol and jump to definition

```bash
cr query find assoc
cr query def calcit.core/assoc
```

### Collect edit context in one call

```bash
cr query context app.main/main! --format json
```

### Locate all call sites before refactor

```bash
cr query usages app.main/main!
```

### Locate by text when you only remember a fragment

```bash
cr query search "reload"
```

## Runtime Code Inspection

For comparison, built-in functions inspect live data and definitions at runtime:

```cirru
let
    Point $ defstruct Point (:x :number) (:y :number)
    p (%{} Point (:x 1) (:y 2))
  do
    ; "Get all methods/traits implemented by a value"
    println $ &methods-of p
    ; 'Get tag name of a record or enum'
    println $ &record:get-name p
    ; "Describe any value's internal type"
    println $ &inspect-type p
```

### Getting Help

Use `cr query --help` for the full list of available query subcommands.

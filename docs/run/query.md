---
title: "Querying Definitions"
summary: "使用 calcit query context/defs/def/type/search/find/usages 聚合 Snapshot 元数据与静态语义"
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
  - "calcit query ns"
  - "calcit query defs"
  - "calcit query def"
  - "calcit query type"
  - "calcit query type-at"
  - "calcit query context"
  - "calcit query find"
  - "calcit query usages"
  - "calcit query search-expr"
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
calcit query ns

# Show definitions in a specific namespace
calcit query ns calcit.core
```

### Read Code (`def`)

```bash
# Show full source code of a definition
calcit query def calcit.core/assoc

# Builtin helpers without snapshot source still return metadata
calcit query def calcit.core/to-js-data
```

For source-backed definitions, `query def` prints the stored Cirru body. For special builtin helpers such as `calcit.core/to-js-data`, it falls back to builtin metadata (doc, schema, examples count) even when no snapshot source exists.

Since 0.14.3, automation should use `calcit query def namespace/name --format json`.
Stdout is one JSON envelope (`schema_version: 1`, `command: "query.def"`, `revision`,
`data`, `diagnostics`); command echoes and warnings remain on stderr. Failures exit
nonzero with diagnostics on stderr, not a partial success object. `data` contains
`id`, `doc`, `tags`, `examples`, `tests`, `code`, `schema`, `ffi`, and `ffi_edn`.
`ffi` is complete EDN-encoded JSON, using the same representation as `cirru parse-edn`:
tag map keys retain `:`, tag values use `{"__edn_tag":"js"}`, and sets use
`{"__edn_set":[...]}`. `ffi_edn` is complete, parseable Cirru EDN text. Both are
`null` when absent; neither is a preview. Source-backed definitions have a content
revision; source-less builtins have `revision: null`, `code: null`, and `builtin: true`.
The schema field remains a Cirru syntax tree, not raw persisted schema data.

兼容性：`--json` 仍在 human 输出末尾附加 `JSON:` 与旧字段对象，其中 `ffi` 保持
字符串类型，但不再截断。`--format json` 优先于 `--json`，不需要 `--raw` 就会返回
完整元数据。human 模式默认标明 FFI preview；`--raw` 同时输出完整代码与 FFI。
本接口只查询声明，不改变 Interface IR v2、目标可用性检查或 async 调用语义。

Local metadata queries (`ns <name>`, `defs`, `def`, `peek`, `examples`, `schema`, `pkg`, and `config`) first read only the main Snapshot. Modules/core are loaded only when the requested namespace is not local. This keeps repeated Agent navigation fast and avoids unrelated dependency warnings; semantic queries such as `type`, `type-at`, and `context` still load the metadata needed for static resolution.

### Peek Signature (`peek`)

```bash
# Show documentation and examples without the full body
calcit query peek calcit.core/map
```

### Check Examples (`examples`)

```bash
# Extract only the examples section
calcit query examples calcit.core/let

# Builtin helpers can also expose curated examples when available
calcit query examples calcit.core/to-js-data
```

To execute stored examples without running an entire namespace's examples, use `calcit analyze check-examples --ns app.main --def target-name`.

### Read Schema (`schema`)

```bash
# Function and value schemas use the same query
calcit query schema app.main/main!
calcit query schema 'app.main/*enabled?'

# Versioned machine-readable envelope with canonical schema and Cirru tree
calcit query schema 'app.main/*enabled?' --json
```

Parameterized value schemas are rendered directly, for example `:: :ref :bool`; they are no longer hidden as `(none)` merely because they are not function schemas. `--json` emits actual JSON, including both the canonical one-line schema and its Cirru tree, rather than a Cirru EDN fragment labeled as JSON.

### Find Symbol (`find`)

```bash
# Search for a symbol across ALL loaded namespaces
calcit query find assoc
```

### Analyze Usages (`usages`)

```bash
# Find where a specific definition is used
calcit query usages app.main/main!
```

### Search Text (`search`)

```bash
# Backward-compatible default: search project, configured dependencies, and bundled core
calcit query search hello

# Bound the inventory to one source class
calcit query search hello --source project
calcit query search hello --source core
calcit query search hello --source deps
calcit query search hello --source all

# Limit to one definition
calcit query search hello --filter app.main/main!

# Stable paths and matched trees in one JSON envelope
calcit query search hello --filter app.main/main! --format json
```

### Search Expressions (`search-expr`)

```bash
# Search structural expressions (Cirru pattern)
calcit query search-expr "fn (x)"

# Limit to one definition
calcit query search-expr "fn (x)" --filter app.main/main!

# `--json` decodes the pattern; `--format json` controls result encoding
calcit query search-expr '["fn",["x"]]' --json --filter app.main/main! --format json
```

`query search` defaults to `--source all` for compatibility. `project` means namespaces stored in the input Snapshot,
`core` means the bundled `calcit.core`/`calcit.internal` Snapshot, and `deps` means modules configured by the selected
entry (`--entry` selects a different complete entry configuration). Source filtering happens before deterministic
definition ordering and global `cursor_index` assignment.

Search JSON results contain a summary plus definition rows with `code@...` paths and the matched Cirru tree. Every
definition and match reports `source` plus an `origin` object containing its package and module path. `node_kind`
is `leaf`, `call`, or `expr`; in particular, a bare `%none` leaf and the callee inside `(%none)` no longer require
parent-tree inference. `--parent-path` also returns the editable parent path for leaf searches. JSON stdout remains
one schema-versioned envelope. With the default source and no explicit `--entry`, a project/core namespace filter
keeps the previous shallow-loading behavior and does not load unrelated dependency modules.

### Inspect Static Type Methods (`type`)

```bash
# Builtin type
calcit query type "'Number"

# Parameterized type; pass Cirru directly, without an extra outer parenthesis layer
calcit query type ":: 'List 'Number"

# A definition with an explicit static schema
calcit query type calcit.core/ceil

# Machine-readable result; stdout is one JSON value
calcit query type "'Number" --format json
```

`query type` loads and preprocesses static metadata but does not run the project init or reload function. It lists methods in dispatch-precedence order and shows the impl that contributes each method. Definition targets first use an explicit schema, then static source inference. This allows `defstruct` and `defenum` declarations with a dynamic entry schema to expose their named type and methods without constructing a runtime value. If neither source is sufficient, query a concrete type annotation instead.

### Inspect an Expression Type (`type-at`)

Use a Snapshot path returned by `query search`, `query context`, or another structural query:

```bash
# Inspect one field-access expression without running the project entry
calcit query type-at test-struct.main/sum-point --path code@3.1

# Machine-readable evidence envelope
calcit query type-at test-struct.main/sum-point --path code@3.1 --format json
```

`query type-at` statically preprocesses the selected definition and reports:

- the inferred type and confidence;
- the expected type supplied by a return schema, callable parameter, `if` condition, or `assert-type`;
- relevant typed bindings and their Snapshot paths;
- statically resolvable methods and implementation origins;
- the source callable and its preprocess lowering status (`specialized`, `resolved`, `dynamic`, or `unavailable`), including the selected core primitive when type information changes the execution path;
- evidence, diagnostics, definition revision, and follow-up commands.

The command does not invoke the project init/reload function. A dynamic FFI boundary is labeled `intentional-js-ffi` when the enclosing schema declares `:features $ #{} :js-ffi`; unresolved expressions remain explicit rather than triggering a runtime fallback. Named `defstruct`/`defenum` values are retained as source-backed type references, so field and method metadata can be resolved without constructing application values.

`type-at --format json` uses protocol `schema_version: 2`. Version 2 adds the required `data.lowering` object with `status`, `kind`, `source_head`, `lowered_head`, and `detail`. Tooling can use `status: specialized` to confirm that rich source syntax reached a type-selected primitive, and should treat `dynamic` or `unavailable` as a migration/checking signal rather than assuming that successful inference implies optimized execution.

### Gather Definition Context (`context`)

```bash
# One bounded view for understanding or preparing to edit a definition
calcit query context app.main/main!

# Return the typed result envelope as JSON
calcit query context app.main/main! --format json

# Use a smaller content budget and include dependency/core usages
calcit query context app.main/main! --budget 2400 --deps

# Builtin helpers without a Snapshot body use curated metadata
calcit query context calcit.core/to-js-data --format json
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
calcit query find assoc
calcit query def calcit.core/assoc
```

### Collect edit context in one call

```bash
calcit query context app.main/main! --format json
```

### Locate all call sites before refactor

```bash
calcit query usages app.main/main!
```

### Locate by text when you only remember a fragment

```bash
calcit query search "reload"
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
    ; 'Get the definition tag name of a struct value'
    println $ &struct:get-name p
    ; "Describe any value's internal type"
    println $ &inspect-type p
```

### Getting Help

Use `calcit query --help` for the full list of available query subcommands.

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

### 管道输出与退出状态

文档读取和 `query` 输出可以交给 `head` 等工具读取前缀，例如
`calcit docs search Fn | head -n 1`。下游提前关闭 stdout 时，Calcit 将
BrokenPipe 视为读者主动结束，安静退出且状态为 `0`，不输出 panic/backtrace。
这不保证截断后的 JSON 或 Cirru EDN 仍是完整文档；需要解析时应读取完整输出，
或优先使用查询命令已有的范围、预算等参数。

其他 stdout 写入错误仍报告到 stderr 并以状态 `1` 退出，不作为成功处理。
该约定用于 CLI 文档/查询展示及其共享渲染函数，不修改 Calcit 程序运行时的
`println`、FFI、watch 或其他执行模式的错误语义。

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

Agent 查询优先显式使用 `--format edn`，例如 `calcit query def namespace/name --format edn`；
需要与 JSON 工具互操作时再指定 `--format json`。`query type`、`type-at`、`context`、
`def`、`config`，以及只读的 `config show/modules/type-slots` 使用相同的结构化输出约定。
其中 `query config --format edn/json` 复用 `config show`，envelope 的 `command` 为
`config.show`，便于收敛旧查询入口。
human 默认仍使用 Markdown。成功与失败的 stdout 各只有一个可解析的 envelope；失败
非零退出，`data` 为 `nil`／`null`，诊断放在 `diagnostics`，命令回显与日志留在 stderr。
EDN 键为 `:schema-version` 等 tag；JSON 对应 `schema_version`。两种格式的身份、
revision、类型事实和诊断语义一致。没有源码函数体的 runtime-only 定义会显示 leaf
占位符并给出 `I_SOURCE_BODY_UNAVAILABLE`，不会因格式化失败丢失整个查询。

```bash
calcit calcit/test.cirru query type "'String" --format edn
calcit calcit/test.cirru query context 'calcit.core/&list:contains?' --format edn
calcit calcit/test.cirru config show --format edn
```

`query def` 的 `data` 包含 `id`、`doc`、`tags`、`examples`、`tests`、`code`、
`schema`、`ffi` 和 `ffi_edn`。`--format edn` 的 `:ffi` 保留原生 Cirru EDN Map/Tag；
`--format json` 的 `ffi` 才使用互操作编码：tag 键保留 `:`，tag 值使用
`{"__edn_tag":"js"}`，set 使用 `{"__edn_set":[...]}`。`ffi_edn` 是完整可解析的
Cirru EDN 文本；缺失时为 `nil`／`null`，两者均不是 preview。有源码定义带内容
`revision`；无源码 builtin 的 `revision`、`code` 为 `nil`／`null`，`builtin` 为 `true`。
The schema field remains a Cirru syntax tree, not raw persisted schema data.

兼容性：`--json` 仍在 human 输出末尾附加 `JSON:` 与旧字段对象，其中 `ffi` 保持
字符串类型，但不再截断。`--format json` 优先于 `--json`，不需要 `--raw` 就会返回
完整元数据。human 模式默认标明 FFI preview；`--raw` 同时输出完整代码与 FFI。
本接口只查询声明，不改变 Interface IR v3、目标可用性检查或 async 调用语义。

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

# Stable paths and matched trees in one Cirru EDN envelope
calcit query search hello --filter app.main/main! --format edn
```

### Search Expressions (`search-expr`)

```bash
# Search structural expressions (Cirru pattern)
calcit query search-expr "fn (x)"

# Limit to one definition
calcit query search-expr "fn (x)" --filter app.main/main!

# `--json` decodes the pattern; `--format edn` controls result encoding independently
calcit query search-expr '["fn",["x"]]' --json --filter app.main/main! --format edn
```

`query search` defaults to `--source all` for compatibility. `project` means namespaces stored in the input Snapshot,
`core` means the bundled `calcit.core`/`calcit.internal` Snapshot, and `deps` means modules configured by the selected
entry (`--entry` selects a different complete entry configuration). Source filtering happens before deterministic
definition ordering and global `cursor_index` assignment.

结构化搜索结果包含摘要、带 `code@...` 路径的定义，以及匹配的 Cirru 树。每条定义与匹配都包含
`source` 和表示 package/module 路径的 `origin`。`node_kind` 区分 `leaf`、`call` 和 `expr`，
因此裸 `%none` 叶子与 `(%none)` 中的被调用项不必再靠父树猜测。`--parent-path` 可返回叶子匹配的
可编辑父路径。EDN stdout 是单个带版本的 envelope；JSON 只作为显式互操作投影。默认 source 且未显式
指定 `--entry` 时，project/core namespace 过滤仍沿用浅加载，不会装载无关依赖模块。

### Inspect Static Type Methods (`type`)

```bash
# Builtin type
calcit query type "'Number"

# Parameterized type; pass Cirru directly, without an extra outer parenthesis layer
calcit query type ":: 'List 'Number"

# A definition with an explicit static schema
calcit query type calcit.core/ceil

# Machine-readable result; stdout is one Cirru EDN value
calcit query type "'Number" --format edn
```

`query type` loads and preprocesses static metadata but does not run the project init or reload function. It lists methods in dispatch-precedence order and shows the impl that contributes each method. Definition targets first use an explicit schema, then static source inference. This allows `defstruct` and `defenum` declarations with a dynamic entry schema to expose their named type and methods without constructing a runtime value. If neither source is sufficient, query a concrete type annotation instead.

### Inspect an Expression Type (`type-at`)

Use a Snapshot path returned by `query search`, `query context`, or another structural query:

```bash
# Inspect one field-access expression without running the project entry
calcit query type-at test-struct.main/sum-point --path code@3.1

# Machine-readable evidence envelope
calcit query type-at test-struct.main/sum-point --path code@3.1 --format edn
```

`query type-at` statically preprocesses the selected definition and reports:

- the inferred type and confidence;
- the expected type supplied by a return schema, callable parameter, `if` condition, or `assert-type`;
- relevant typed bindings and their Snapshot paths;
- statically resolvable methods and implementation origins;
- the source callable and its preprocess lowering status (`specialized`, `resolved`, `dynamic`, or `unavailable`), including the selected core primitive when type information changes the execution path;
- evidence, diagnostics, definition revision, and follow-up commands.

The command does not invoke the project init/reload function. A dynamic FFI boundary is labeled `intentional-js-ffi` when the enclosing schema declares `:features $ #{} :js-ffi`; unresolved expressions remain explicit rather than triggering a runtime fallback. Named `defstruct`/`defenum` values are retained as source-backed type references, so field and method metadata can be resolved without constructing application values.

`type-at --format edn` 使用版本 2 的结构化协议，包含 `:data :lowering` 中的 `:status`、`:kind`、
`:source-head`、`:lowered-head` 和 `:detail`。显式选择 JSON 时，对应键为 `schema_version`、
`data.lowering.source_head` 等兼容形式。`specialized` 表示源码语义已经选择类型专门化的 primitive；
`dynamic` 或 `unavailable` 仍需作为迁移和检查线索，不能因为推断成功就假定执行路径已优化。

### Gather Definition Context (`context`)

```bash
# One bounded view for understanding or preparing to edit a definition
calcit query context app.main/main!

# Return the typed result envelope as Cirru EDN
calcit query context app.main/main! --format edn

# Use a smaller content budget and include dependency/core usages
calcit query context app.main/main! --budget 2400 --deps

# Builtin helpers without a Snapshot body use curated metadata
calcit query context calcit.core/to-js-data --format edn
```

`query context` combines information that otherwise requires several commands:

- definition identity and deterministic revision;
- Snapshot doc, tags, schema features, examples, and a bounded code preview;
- 小型代码或示例的结构化语法树；若省略则提供后续查询命令；
- trusted type-coverage state and static methods;
- direct dependencies and usage locations such as `code@3.2`;
- unresolved versus intentional (`:js-ffi`) dynamic-type diagnostics;
- suggested next commands for selectively expanding truncated sections.

The numeric paths are scoped to the returned revision. Re-query after a change before using a path for editing. `--budget` is an approximate character budget for variable-size content; explicit `--dependency-limit`, `--usage-limit`, and `--example-limit` bounds are also available.

选择 `--format edn` 时，stdout 只包含一个 Cirru EDN envelope；JSON-only consumer 可显式选择
`--format json` 获取对应的 JSON envelope。命令说明与平台注册信息留在 stderr，不应和结构化结果混读。

## Quick Recipes (for fast locating)

### Locate a symbol and jump to definition

```bash
calcit query find assoc
calcit query def calcit.core/assoc
```

### Collect edit context in one call

```bash
calcit query context app.main/main! --format edn
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

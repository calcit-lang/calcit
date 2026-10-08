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

### 命名空间签名概览（`defs --signatures`）

```bash
calcit query defs app.main --signatures
```

每行列出定义名、签名与 doc 首行：有 schema 的显示单行 schema，没有 schema 的显示 `(untyped)` 与声明头（如
`(untyped) defn helper $ x`），值本身不进入概览。不带 `--signatures` 时只用 `[schema]` 标记是否声明了 schema。
了解一个模块时先看这个概览，再按需对单个定义使用 `schema`、`examples`、`context`。

### Read Code (`def`)

```bash
# Show full source code of a definition
calcit query def calcit.core/assoc

# Builtin helpers without snapshot source still return metadata
calcit query def calcit.core/to-js-data
```

For source-backed definitions, `query def` prints the stored Cirru body. For special builtin helpers such as `calcit.core/to-js-data`, it falls back to builtin metadata (doc, schema, examples count) even when no snapshot source exists.

`query type` 给出 `.method` 的类型契约，方法后面的 definition path 只是实现入口；`query def/context` 的 `:internal` 标签不能被误读为公开推荐。对已存在显式 fix 且旧、新方法在具体接收者上都证明为同一实现、同一参数和返回契约的别名，查询附带 `role: preferred` 或 `role: compatibility`；兼容名还有 `preferred-name` 与 `fix-rule`（Cirru EDN 拼写；JSON 字段使用下划线）。`proven` 只表示类型契约可用，**不等于首选名称**。开放、歧义或不同实现的方法不会凭拼写获得推荐角色；准确查询旧名仍可见原实现。区分 nominal 类型名、构造器和内部函数时，参见 [API 命名角色](../features/api-roles.md)。

`query def/context` 对可直接证明为已注册 core Proc 的定义另给 `:runtime-arity`（JSON 为 `runtime_arity`）：`:min` 是最少传参个数，`:max` 是最多个数，`nil` 表示没有有限上限。例如 `read-dir` 为 `{:min 1, :max 2}`，省略第二个参数不意味着可以传 nil，也不意味着它的 Bool 值类型变为 Optional。human 输出显示 `Runtime Proc arity: 1..=2`。此证据复用编译器已有 Proc 元数据，不根据函数名、schema 宽度或 `:internal` 标签猜测；普通函数、未知别名和特殊 builtin 未证明时不提供该字段。字段缺失不是零参数、无上限或可调用的承诺。参数值类型、失败与 backend 支持仍需结合 schema、文档与测试查看，arity 不代替它们。

同一类 core Proc 定义另给 `:failure`，说明参数已符合声明类型时调用如何失败：`:class` 为 `|total`（总能返回值）、`|result`（通过 nil、Option 或 Result 返回预期失败）或 `|raises`（抛出可由 `try` 捕获的错误，WASM 中 trap），`:condition` 写明返回失败或抛错的条件。类型不符的参数在所有 Proc 上都是类型错误，不重复列出。human 输出显示为 `Failure: ...` 一行：

```bash
calcit query def 'calcit.core/&list:nth'
# - Failure: `raises` when an index is not a non-negative integer or is out of range
calcit query def 'calcit.core/&map:get' --format edn
# :failure $ {} (:class |result) (:condition "|nil for a missing key")
```

分类与 Proc 实现放在一起，新增 Proc 必须同时给出类别，否则无法编译。需要可恢复的失败时，优先选择 `result` 类入口或对应的 Option/Result 方法；`raises` 类调用只在条件已由前面的代码排除时使用。

默认 Markdown 输出先列已证明的首选方法，再列尚未完成角色归类的方法，最后列附有首选名和 fix 规则的兼容方法。分组仅改变阅读顺序，不隐藏方法、不改变分派优先级或结构化输出；尚未归类不表示应优先使用。

例如 `query type ":: 'List 'Number" --format edn` 会把单元素追加的 `.append`、长度查询的 `.len` 标为首选，旧 `.add/.count` 分别关联 `core-list-add-v1`、`core-collection-len-v1`。同一长度规则也覆盖契约已证明的 Map/Set/String；Map `.add` 是不同的 entry 操作，不会因此变成 `.append` 或 `.assoc` 的兼容别名。

命名整理的已实现入口与未来目标必须区分：用 `calcit docs read api-roles.md '逐族命名决策'`
查看整组契约，或把章节改成 `谓词与成员查询`、`集合长度、组合与遍历` 缩小输出。
标为“目标”的名字只有在当前版本的查询中实际存在且类型契约可用时才可生成调用；
`open` 表示查询证明不足，不能直接等同于运行失败，也不能绕过严格检查。

结构化方法条目中的 `:call-types`（JSON 为 `call_types`）补充已解析的参数、rest 和返回类型语法节点；参数不含隐式接收者，Fn 回调及泛型变量不会退化成显示字符串。它复用已有 checker 的证明，只在 `proven` 且语法可表达时提供；`open/ambiguous` 没有该字段。原有 `parameter-types/return-type` 仍便于阅读，schema 原声明仍通过 definition path 查询。比如 List `.get` 的实现 `get` 可处理开放值，但具体 List 查询给出的调用返回类型是 `Option<Number>`，不能只凭实现的宽 schema 判断方法类型。默认 human 输出保持现有紧凑分组，不增加命令或参数。

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

`--format cirru` 只输出 `quote $ <定义>` 形式的源码，不带 Markdown 标题、Schema 段或代码围栏，可以直接作为
`edit def --overwrite --input-format cirru` 的输入。读出的视图未经修改写回时，Snapshot 字节保持不变；
schema、doc、examples 与 tests 不在该视图中，写回代码时保持原值。`--raw` 是不分块的 Markdown 输出。

```bash
calcit query def app.main/main! --format cirru > .calcit/snippets/main.cirru
# 修改 .calcit/snippets/main.cirru 后写回
calcit edit def app.main/main! --overwrite --input-format cirru --file .calcit/snippets/main.cirru
```

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

`query type` 只加载和预处理静态元数据，不执行项目的 init/reload 函数。默认 Markdown 按已证明首选、未归类、兼容入口分组；显式 EDN/JSON 保持原有方法列表和分派优先级，每项仍可追溯贡献该方法的实现。查询定义时先使用显式 schema，再尝试静态源码推断。因此，即使 `defstruct`、`defenum` 的 entry schema 为 Dynamic，也能在不构造运行时值的情况下查看名义类型及方法；若两种证据都不足，请查询具体类型标注。

通过 `(requires Parent)` 获得的方法也列在同一个 `methods` 中，`origin` / `definition` 指向实际声明它的父 trait；传递继承和重复路径复用正常静态分派的解析，不生成另一套方法注册表。普通 trait 与 external-object trait 使用同一规则。例如：

```bash
calcit calcit/test-traits.cirru query type test-traits.main/Greeting --format edn
calcit calcit/js-ffi-consumer.cirru query type app.main/TallyHost --format edn
```

前者同时列出自身的 `.greeting` 和父 trait 的 `.label`；后者列出父 trait 的 `.add!`，字段 `:total` 不冒充方法。`proven` 仍只证明声明的调用契约，不代表 JavaScript 宿主的任意实现经过运行时验证；包含 Dynamic / DynFn 的签名保持 `open`，不同名义来源的同名方法保持 `ambiguous`。

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

`:next` 中的源码导航命令保留本次查询的 Snapshot，并对路径与 definition 做 shell quoting。
结构化 `:data :path` 继续使用 `code` / `code@3.1`；对应 `tree show` 命令在根节点省略 `--path`，
子节点使用原有 `@3.1` 数字坐标。可直接复制下一步命令，不必手动转换路径；该导航只读，不改写源码。

### Gather Definition Context (`context`)

对于未声明 root schema、但普通预处理已证明签名的 helper，结构化输出保留 `:schema nil`，另外提供 `:inferred-schema`；human 输出单独显示 “Inferred schema”。它是编译器证据，不是源码声明，查询不会写回 Snapshot。此时不会再仅因缺少 root schema 报告未解析类型；实际预处理错误仍会出现在 diagnostics 中。

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

默认的依赖列表不包含 `fn`、`let`、`if` 等 core 宏与语法，函数定义的 static methods 也不列出所有函数共有的
`.apply`、`.bind`、`.call` 等通用 Fn 方法；`total` 计数同样按过滤后的结果。需要完整列表时传 `--include-core`。

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

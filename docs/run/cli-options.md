---
title: "CLI Options"
scope: "core"
kind: "reference"
category: "run"
aliases:
  - "watch mode"
  - "watch"
  - "once mode"
  - "check-only"
  - "keep-going"
  - "reload-fn"
  - "reload fn"
  - "watch-dir"
  - "macro metrics"
  - "macro expansion metrics"
  - "ffi metrics"
  - "async ffi metrics"
  - "ffi export"
entry_for:
  - "calcit -w"
  - "calcit js -w"
  - "calcit --help"
  - "calcit --reload-fn"
  - "calcit --check-only --keep-going"
  - "calcit --macro-metrics"
  - "calcit --ffi-metrics"
  - "calcit ffi export"
---

# CLI Options

```bash
calcit --help
```

Quick note: `calcit edit format` rewrites the target snapshot using canonical serialization without guessing semantic changes. The retired `compact.cirru` filename must first be copied or renamed to `calcit.cirru`. Formatting alone has an isolated one-way loader for early direct-quote definitions/namespaces and top-level `:configs`; runtime loading and other commands remain strict. It reports migrated node counts and rejects ambiguous or unknown legacy config fields. Migrated ordinary definitions receive explicit `Dynamic` schemas. A migrated direct-quote `defmacro` instead receives a conservative strict contract recovered from its parameter shape: `Syntax` inputs, an `Expr<Dynamic>` expansion, and no capabilities. Existing structured Dynamic macro schemas are not rewritten. Current formatting also normalizes older namespace records and rewrites legacy schema type tags such as `:string` and `:ref` to quoted symbols such as `'String` and `'Ref` only in known type positions. Ordinary tag data stays unchanged. It emits recoverable stderr advisories for legacy `:any` and unresolved dynamic type debt. Use `calcit analyze weak-types` for exact paths and recommendations; format warnings do not turn the command into a type-quality gate.

问题写法检测使用显式 fix preview。例如
`calcit calcit.cirru fix --rule redundant-do-v1 --format edn` 会报告多表达式 body 中不必要的 `do`，但不会写入文件；
审阅建议后使用报告中的 revision 和相同 scope selectors 执行 `--apply`。不要把这类源码整理混入普通类型 warning，
也不要通过正则批量删除仍在单表达式位置承担顺序语义的 `do`。完整流程见 [Compiler-guided Source Fixes](fix.md#检测并修复冗余-do)。

仍需旧规则的项目先按[历史两阶段升级流程](upgrade-history.md#01415-的两阶段源码修复桥接)固定使用 Calcit 0.14.15 运行 `tag-match-to-match-v1` 与
`required-struct-field-v1`；验证并提交后，再使用当前工具链运行
`calcit calcit.cirru fix --preset surface-latest-v2 --format edn`。该 preset 会在 Cirru EDN 的
`:data :filters :expanded-rule-ids` 中列出冻结的规则集合，包括冗余 `do`、旧数据 API，以及可静态证明安全的具名
Enum/Struct `%::` / `%{}` 直接构造器迁移，并解包普通可执行源码中的单表达式 `do`。冻结的 v1 仍可复现原有四条规则。
`--preset` 与 `--rule` 互斥；apply 必须重复同一 selector。

整个项目的严格升级使用同一个入口：
`calcit calcit.cirru fix --workflow strict --format edn` 生成包含 entries/type slots、安全 fixes、类型与 FFI review
边界、verification 命令及恢复 revision 的 manifest；审阅后加 `--apply --expect-revision <revision>`，最后运行
`--workflow strict --verify`。该 workflow 不会自动决定 schema、Dynamic 收窄、FFI trust 或业务默认值，也不会猜测
外部包管理器。

需要集中审阅 JavaScript 边界时，使用已有分析入口：
`calcit calcit.cirru analyze weak-types --ffi-evidence --format edn`。该选项按 operation 标记 browser、node、
npm-import、webgpu 或 unknown-host，列出静态 caller、nullable/unsafe 路径，并给出 exact-schema helper、trait 与
adapter 候选。它只整理 Snapshot、schema 和 namespace import 中可证明的证据；候选均为 `review-required`，不会推断
运行时 trust，也不会自动收窄 Dynamic、选择 Option/Result 或写入源码。默认不启用该扫描，避免把迁移证据变成新的
类型门禁；JSON 仅作为互操作格式。

需要审阅 schema 与数据建模候选时，仍使用同一个入口：
`calcit calcit.cirru analyze weak-types --schema-evidence --format edn`。它复用 `synthesize-schema-v1` 的编译器和
resolver call-site 证据，并列出重复匿名 Map shape 与 `match` tag dispatch。输出带置信度、冲突、unknown slot 和
源码路径，全部是 `review-required`；不会自动创建 Struct/Enum，也不会决定业务默认值或错误语义。

在频繁 edit/check/analyze 循环中，可为直接入口检查使用 `calcit calcit.cirru --check-only --incremental`，或为
`analyze check-types`、`analyze weak-types`、`analyze dynamic-methods` 增加各自的 `--incremental`。这些参数不新增命令，
在项目 `.calcit/analysis-input-cache-v2.cirru` 分别复用由内容 digest 校验的主 Snapshot 与 direct module 单元，并在
`.calcit/analysis-cache-v1.cirru` 保存按 definition revision 分离的本地结果；缓存默认使用 Cirru EDN，JSON 仅保留为显式
互操作输出。报告会分别返回 `cache.input.entry` 与 main/module hit/miss、definition hit/miss，以及 `cache.dependency_index` 的编译器解析依赖索引摘要。
顶层 `--entry` 会先选择对应配置，再校验和加载该 entry 的 modules；切换 entry 或严格模式默认 feature policy 时，definition cache
会按 context revision 冷启动，不会沿用另一 entry 的分析结果。
依赖索引按 definition 与 namespace revision 复用，变化时报告 changed/affected 数量；affected 包含反向传递调用者。
`dynamic-methods --incremental` 会进一步校验 init/reload 的完整 dependency closure：闭包不变时复用动态方法诊断并明确报告
`preprocessing_cached=true`，闭包索引不完整时保守执行完整入口预处理。它不缓存 compiled AST，也不替代严格类型检查。
`--check-only --incremental` 只复用相同 closure、strict policy 与 dynamic-method policy 下已经成功的入口预处理；它只保存成功标记，
warning/error 不缓存。reachable definition/schema、namespace import、entry type-slot、policy、编译器或 core input 变化都会冷启动；闭包证据
缺失或含 unresolved dependency，以及 module load 失败时会执行普通检查或明确 bypass，均不会复用或更新旧的成功标记。`--keep-going`
需要逐 definition 重新收集结构化诊断，因此不能与该成功缓存组合。
单个模块的传递输入变化只重载该模块，其他模块和未变化 definition 的结果仍可命中。
input cache 头损坏或版本不一致时会从源码重载主 Snapshot 和 modules；单个缓存单元失效时只重载该单元，有效的 definition 缓存仍可命中。
input cache 写入失败时本次分析继续，但不会更新该缓存；definition cache 缺失、不可读，或因 schema、编译器版本、context revision
变化而失效时，才会让 definition-local inventory 冷启动。它目前不缓存 schema evidence、deprecated 或 quality；最终 CI 仍应运行无缓存检查。

For feature-level planning, use `calcit edit scaffold`. Its primary input is a
Cirru EDN architecture plan, preferably stored under
`docs/architectures/<feature>.cirru`:

```bash
calcit calcit.cirru edit scaffold --file docs/architectures/order.cirru \
  --dry-run --format edn
calcit calcit.cirru edit scaffold --file docs/architectures/order.cirru \
  --expect-revision md5:... --format edn
```

`--dry-run` previews reconciliation and work items without writing. Apply mode
atomically creates missing definitions only; existing definitions are reported
with their planned/existing metadata and are never overwritten. EDN is the
canonical machine format; JSON is a compatibility projection.

## Detailed Option Descriptions

### Typed FFI inventory (`ffi export`)

Export local raw bindings declared by non-empty `:ffi` lowering metadata:

```bash
calcit calcit.cirru ffi export
calcit calcit.cirru ffi export --json --ns app.ffi
calcit calcit.cirru ffi export --boundary component
calcit calcit.cirru ffi export --boundary component --format json
```

默认 `native` boundary 保持现有 human inventory 和 `--json` 兼容性。
`--boundary component` 只选择 typed `defwasm-import` / `defwasm-export`，
记录 direction 和 binding identity，并默认输出 Cirru EDN。使用
`--format human|edn|json` 显式选择表达。

结构化模式在 stdout 只输出一份可解析文档，unsupported schema
以确定诊断返回，不退化为 Dynamic fallback。native raw-binding schema
见 [FFI Interface IR](../installation/ffi-interface-ir.md)；Component contract 与职责边界见
[WASM Component 边界](../installation/wasm-component-boundary.md)。

### Input File

```bash
# Run default calcit.cirru
calcit

# Run specific file
calcit calcit.cirru
```

### Run Mode (default once)

By default, `calcit` runs once and exits. Use `--watch` (`-w`) to enable watch mode:

```bash
calcit --watch
calcit -w calcit.cirru
```

### Error Stack Trace (--disable-stack)

Disables detailed stack traces in error messages, useful for cleaner output:

```bash
calcit --disable-stack
```

### JS Codegen Options

**--skip-arity-check**: When generating JavaScript, skip arity checking (use cautiously):

```bash
calcit js --skip-arity-check
```

**--emit-path**: Specify output directory for generated JavaScript:

```bash
calcit js --emit-path dist/
```

### Dynamic Method Warnings (--warn-dyn-method)

Warn when dynamic method dispatch cannot be specialized at preprocess time, and surface related trait-attachment diagnostics:

```bash
calcit --warn-dyn-method
```

In compatibility mode, Dynamic receivers used with nominal Option/Result methods
continue to report `W_DYNAMIC_NOMINAL_METHOD_RECEIVER`. Strict mode rejects the
same ambiguous execution path with `E_DYNAMIC_METHOD_DISPATCH` for prefix calls
or `E_DYNAMIC_POSTFIX_METHOD` for postfix calls. Give the receiver a concrete
`Option<T>` / `Result<T, E>` schema, or use the matching visible `option:*` /
`result:*` function inside an explicitly reviewed open adapter.

Strict mode also promotes every remaining unspecialized project method to the
same stable prefix/postfix errors. The diagnostic classifies the receiver as a
missing schema, Dynamic value/callable, legacy Optional, unbound generic or
type slot, or explicit `:js-ffi` Dynamic boundary. Add static nominal or trait
evidence before method syntax. A JS boundary must convert the host value or
attach an external-object trait inside its narrow adapter; `:js-ffi` alone does
not authorize runtime method lookup. Legacy Optional means an Optional chain
whose payload is an open Dynamic value; `Optional<DynFn>` is classified as a
dynamic callable instead.

默认严格诊断进一步要求 `unsafe-coerce` 位于当前函数声明了 `:features $ #{} :js-ffi` 的结构化 `Fn` schema 内，
不受代码生成模式或兼容 feature policy 影响。否则预处理报 `E_UNSCOPED_UNSAFE_COERCE`；命名空间名称不能代替能力声明。
只有显式运行 `analyze quality` 时，经过作用域检查的转换才会计入逐 definition 的 `unsafeCoerce` 质量 baseline；
严格诊断本身不运行数量预算。

### 默认严格诊断、入口预检查与兼容模式

Calcit 0.14 起，普通调用默认启用严格预处理诊断。项目无需额外开关，也会对无法证明安全的类型操作收到稳定的 `E_*` 诊断：

```bash
calcit --check-only
calcit js
```

默认 `--check-only` 遇到第一个阻断错误就停止，适合本地快速反馈。迁移旧项目或供 Agent 一次收集独立问题时，
在同一个入口增加 `--keep-going`：

```bash
calcit calcit.cirru --check-only --keep-going
calcit calcit.cirru --check-only --keep-going --format edn
calcit calcit.cirru --check-only --keep-going --format json
calcit calcit.cirru --check-only --incremental
```

keep-going 只在 definition 边界恢复：先按静态可达依赖顺序检查，已确认失败的依赖会让调用者标记为
`blocked`，不会把未知后续类型重复报告成新错误；无法归属当前 definition 的意外失败单独标记为
`cascaded`。它不在 expression 内猜测恢复点。human 输出是 Markdown-compatible 文档；结构化输出为单一
envelope，Calcit 自动化优先使用 Cirru EDN，只有 JSON-only consumer 才显式选择 JSON。任一
`failed`、`blocked` 或 `cascaded` 结果都保持非零退出码。`--format` 仅与
`--check-only --keep-going` 组合使用，普通 `--check-only` 的 fail-fast 输出与行为不变。keep-going
只收集严格预处理诊断，不混入基于统计预算的 quality gate；批量问题修复后，可用普通
`--check-only` 再做 fail-fast 检查。

频繁的小步迁移可使用 `--check-only --incremental`。当活动 entry 的 init/reload dependency closure、严格策略与动态方法策略均未变化，
该模式复用上一次成功结果，并以 `preprocessing-cached=true` 明确报告；闭包外定义变化不强制重跑。缓存不保存 compiled AST、warning
或 error，失败检查不会成为后续命中依据。reachable definition/schema、namespace import、entry type-slot、feature policy、编译器或 core input
变化会冷启动。闭包证据缺失或含 unresolved dependency，以及 module load 失败时会执行普通检查或明确 bypass，均不会复用或更新旧的成功标记。
`--keep-going` 仍用于收集完整结构化诊断，两者不可组合；CI 与发布门禁继续运行不带 `--incremental` 的冷检查。

`--strict-types` 可显式确认严格策略，并在执行或代码生成前预检查所选入口；它不声明项目没有 Dynamic 或其他统计债务：

```bash
calcit --check-only --strict-types
calcit --strict-types js
```

所选 entry 未设置 `:feature-policy :js-ffi` 时，默认严格诊断在内存中采用 `:error`，不改写 Snapshot。
旧 entry 可显式设置 `calcit config set feature-policy.js-ffi warn`（或 `allow`）分阶段迁移，并用
`calcit config show` 核对策略。默认严格诊断也包含定位到源码的未类型化 JS FFI 检查。
开放数据边界可以保留经过审阅的 `Dynamic`；是否能进入具体类型操作由编译器 warning/error 决定，
不由 `analyze quality` 的数量预算决定。已有项目仍可显式运行该迁移期报告，但它不是严格检查的一部分。

Use `--compat-types` only as a temporary migration escape hatch when an older
project still needs the pre-0.14 warning behavior:

```bash
calcit --compat-types --check-only
calcit calcit.cirru --compat-types js
```

`--compat-types` disables the default strict diagnostic promotion and the
implicit JS FFI `:error` policy. It does not erase explicitly configured entry
policies. It cannot be combined with `--strict-types`; the CLI rejects that
contradictory request.

Strict preprocessing also rejects two constructs that manufacture `nil`
implicitly:

- `E_LEGACY_OPTIONAL_PARAM`: a `?` parameter would bind an omitted argument to
  `nil`. Remove the marker, declare trailing parameters as `Option<T>`, and let
  omission insert `%none` (or pass `%some value` / `%none` explicitly).
- `E_PARTIAL_STRUCT_NIL_FILL`：`%{}?` / `&%{}?` 已退役，所有模式都拒绝隐式
  `nil` 填充。改用 `%{}` 明确提供字段；可缺失字段应声明为 `Option<T>` 并使用 `%none`。
- `E_NIL_FOR_UNIT`: a function declared to return `Unit` actually returns the
  distinct `Nil` value. Replace returned `nil` / `;nil` with `&unit`, or end the
  body with an effect that already returns Unit. Intermediate nil expressions
  are not classified as the function return.
- `E_NIL_CALLBACK_SENTINEL`: an inline `map-kv` callback has a structurally
  visible return path that uses `nil` to drop an entry. Use `filter-map-kv` and
  return `MapEntryDecision :keep key value` or `MapEntryDecision :drop` on every
  path. A nil nested inside the returned pair remains map data.

Strict typing separately gates the unproven legacy `map-kv` result contract;
these diagnostics are not limited to callbacks that return nil:

- `W_MAP_KV_UNPROVEN_CONTRACT`: compatibility-mode typed project code called
  legacy `map-kv`. Its pair/drop protocol exposes only `Dynamic`;
  migrate to `filter-map-kv` and `MapEntryDecision` for a Map result, or
  `map-list-kv` for a typed `List<U>` result.
- `E_MAP_KV_UNPROVEN_CONTRACT`: strict-mode form of the same migration gate.
  It rejects both prefix `map-kv` and postfix `.map-kv` calls rather than
  allowing an expected result type to invent unproven key/value bindings.

These compatibility paths remain available only with `--compat-types` during
ecosystem migration. Partial Struct construction is not auto-fixed because the
compiler cannot infer whether an omitted business field should become
`Option<T>`, gain a default, or be supplied by the caller.

Strict preprocessing also reports `E_BARE_CONTAINER_SCHEMA` when a public
function or macro contract uses bare `List`, `Map`, `Set`, or `Ref`. The error
includes the exact schema path whose missing type argument became Dynamic. Add
concrete arguments, use a declared `:generics` variable when positions are
related, or spell `Dynamic` explicitly only for a reviewed open boundary. For
example, migrate `List` to `(:: List T)` with `:generics ([] T)`; an intentional
open boundary is written `(:: List Dynamic)` so analysis can distinguish it
from accidental omission.

`E_UNBOUND_TYPE_SLOT` rejects a reachable function or macro contract that uses
`*slot` without a binding in the selected entry. Bind a concrete nominal type
with `calcit config set-type-slot :slot namespace/definition`. When the entry
deliberately opts out of static checking at that boundary, bind `:dynamic`
explicitly; it remains visible to `analyze weak-types` and quality baselines but
is no longer confused with an omitted configuration. Compatibility mode keeps
the existing warning/inventory behavior while projects migrate.

`E_WHOLE_DYNAMIC_PUBLIC_SCHEMA` rejects a reachable project function when
neither its root schema nor an embedded `Fn` hint provides a structured
contract. It also rejects a programmatically supplied macro that reaches
preprocessing without a structured root schema; a nested function hint is not
macro-contract evidence. Replace a missing or whole-`Dynamic` root with a
structured `Fn` or phase-aware `Macro` contract. Existing embedded `Fn` hints
remain valid function evidence during Snapshot migration. Normal Snapshot-loaded macros are validated earlier: a
legacy runtime `Fn` or whole-`Dynamic` macro schema fails during loading with its
definition path. When a boundary is genuinely open, keep `Dynamic` only in the
specific argument, return, or `Expr<Dynamic>` position so `analyze weak-types`
can report and baseline that exact decision. Compatibility mode continues to
inventory these definitions while their schemas are migrated.

`E_ERASED_GENERIC_RELATION` rejects a project call when a `Dynamic` argument
occupies a declared generic position that is related to another argument,
variadic item, nested type, or return position. For example, passing `Dynamic`
to `Fn<T>(T) -> T` prevents strict preprocessing from proving the promised
input/output relationship. Narrow or validate the value before the call. If
the callee is intentionally open, put that operation behind a small adapter
whose structured contract does not claim the generic relationship. With
`--compat-types`, the existing compatibility behavior is unchanged.

当公开集合函数使用 `C: Mappable` 一类 trait facade 时，receiver 的具体 payload
关系可能要等到 List、Set、Map、Option 或 Result 实现被选中后才完整。严格模式会把
这份 receiver-specialized contract 交回同一套类型 proof：`List<Dynamic>` 可以交给
继续接受并返回开放值的 callback，但不能交给只接受 `Number` 等具体类型的 callback。
失败仍使用 `E_ERASED_GENERIC_RELATION`，并报告 callee、参数位置、实际 callback、
特化后的期望契约以及显式 decode/narrow 建议；这里不另设 callback 扫描器或 warning。

If a typed operation directly introduced the open value, the error includes a
single bounded origin and migration direction. The first supported trace is
`Map<K,Dynamic> -> get -> Option<Dynamic> -> generic consumer`: human output
names the receiver and source path, while JSON diagnostics include a
`provenance` array with `operation`, `definition`, `path`, `type`,
`output_type`, `flow`, and `migration`. Direct `Dynamic` arguments do not gain
a speculative trace.

`E_DYNAMIC_NOMINAL_ARGUMENT` rejects a project call when an explicitly open
`Dynamic` value, or a matching container with a `Dynamic` member, enters an
argument whose contract contains a closed Struct or Enum. Decode text with
`parse-cirru-edn-as` / `try-parse-cirru-edn-as`, decode an evaluated host value
with `decode-map-as` / `try-decode-map-as`, or validate and narrow it inside a
small typed FFI adapter. The diagnostic identifies the argument and target
contract. It does not reject unrelated `Dynamic` to primitive calls, and
compatibility mode keeps the existing gradual migration behavior. A type slot
bound to a Struct/Enum contract is resolved before this check, so entry-level
and scoped slot configuration cannot erase the nominal boundary. Cyclic slot
bindings are treated conservatively as protected boundaries instead of being
followed recursively or allowed to bypass the strict check.

Default strict diagnostics report `E_RAW_PRIMITIVE_IN_TYPED_CODE` for hand-written
`&get-raw`, `record-get` / `&struct:get`, raw `&%{}`, and `&struct:nth` without matching
nominal layout evidence. Use Option-returning collection lookup, named Struct
field syntax, and the public `%{}` constructor. Core/reviewed macro lowering,
reusable `defimpl` access, evidence-complete persisted constructors whose fields
exactly match one concrete Struct, and indexed IR whose index/tag agrees with
the concrete receiver layout remain valid.

For a focused, machine-readable inventory that excludes unrelated type and FFI warnings, use the dedicated analysis command:

```bash
calcit analyze dynamic-methods
calcit analyze dynamic-methods --summary-only --format json
calcit analyze dynamic-methods --deps
```

默认只查看项目命名空间；`--deps` 包含可达依赖。此命令始终是只读定位报告，不能用命中数量判断类型正确性；CI 应运行默认严格 `--check-only`。

要在当前 entry target 下检查显式选择的公开 namespace 中全部定义，使用：

```bash
calcit --entry node calcit.cirru analyze check-public \
  --ns package.shared --ns package.node --format json
```

`check-public` 要求至少一个精确 `--ns`，且 entry 必须声明 target。未声明 `:ffi :target` 的定义为共享定义；目标不匹配会在预处理前失败。默认只接受项目 namespace，显式 `--deps` 才允许已加载的依赖与 core namespace。零匹配和不完整检查都会失败。JSON schema version 1 返回已检查的定义 ID、逐定义状态、诊断、完整性、耗时及 scope revision；`--summary-only` 只省略详细结果。内置 core 的 `:builtin` 运行时占位符报告为 `intrinsic`，它们没有 Calcit 函数体，其他源码定义仍严格预处理。

### Macro Expansion Metrics (--macro-metrics)

Use opt-in macro metrics to profile compile, check, and hot-reload work without
changing normal CLI output:

```bash
cargo build --release --bin calcit
target/release/calcit --macro-metrics --check-only calcit/test.cirru
target/release/calcit --macro-metrics --check-only /path/to/respo/calcit.cirru js
```

The CLI writes one `macro-expansion-metrics: {...}` JSON record to stderr when
it exits. Timing fields use nanoseconds. Per-macro and total evaluator and
post-preprocess times are exclusive: when a nested macro starts, its parent's
timer pauses, so totals do not double-count recursive expansion work.

The report records general-evaluator fallbacks, cache misses, miss reasons, and
bypass reasons. Watch mode keeps a conservative raw-expansion cache for macros
with strict signatures and no compile-time capabilities. Entries are scoped to
stable source call sites and validate the macro identity, signature, exact input
syntax (including locations), and gensym sequence before reuse. Legacy,
effectful, runtime-evaluator, unstable-call-site, and non-watch calls report an
explicit bypass reason. `cacheInvalidations` separates changed macro definitions,
signatures, inputs, and gensym sequences; a cache hit skips macro evaluation but
still preprocesses and type-checks the emitted expansion.

The cache deliberately targets repeated preprocessing during hot reload. A
normal once-mode build does not populate it, avoiding cold-build memory and
cloning overhead. It does not cache post-preprocess results yet, so helper/import/
type dependency invalidation for that higher-ceiling optimization remains future
work.

Reproducible release-mode baseline from 2026-08-25 (three warm runs, median):

| Project | Revision | Expansions | Evaluator | Post-preprocess | General fallback |
| --- | --- | ---: | ---: | ---: | ---: |
| Calcit test snapshot | `32883223` | 2,432 | 20.21 ms | 52.61 ms | 2,432 |
| Respo JS check | `be8141e` | 1,414 | 14.24 ms | 35.50 ms | 1,414 |

Calcit's highest post-preprocess costs in the median run were `assert=`
(21.66 ms), `let` (10.16 ms), `def` (5.39 ms), and `fn` (4.20 ms). Respo's
were `let` (11.53 ms), `def` (4.71 ms), `fn` (2.92 ms), and `cond` (1.87 ms).
These results prioritize common structural macros and post-expansion processing
for the typed Macro IR phase; they do not claim application runtime gains.

### Native Async FFI Metrics (--ffi-metrics)

Use opt-in native async FFI metrics when a server or integration test needs a
machine-readable backpressure and lifecycle sample:

```bash
calcit --ffi-metrics --entry server
```

On normal exit or bounded Ctrl-C shutdown, the CLI writes exactly one
`ffi-async-metrics: {...}` JSON record to stderr. Business stdout remains
unchanged. The versioned report contains totals and stable module/method rows
for active, closing, and completed tasks; current queue depth, bytes, and oldest
age; enqueue/coalesce/queue-full/dequeue/purge counts; response deadline
timeouts; and cancellation requests, successes, and failures. Completed tasks
are folded into bounded module/method aggregates rather than retained one by
one. No payload is copied into or printed by the report.

使用 `--ffi-metrics` 可在服务或集成测试正常退出、或完成有界 Ctrl-C 关闭时，
向 stderr 输出唯一一条 `ffi-async-metrics: {...}` JSON 记录，不改变业务
stdout。报告按 module/method 汇总当前与已完成 task、queue backlog、oldest
age、enqueue/coalesce/queue-full/dequeue/purge、response deadline timeout 与
cancel 请求/成功/失败；已完成 task 只进入有界聚合，不逐项长期保留，也不会
复制或打印业务 payload。

### Hot Reloading Configuration

**--init-fn**: Override the main entry function:

```bash
calcit --init-fn app.main/start!
```

**--reload-fn**: Specify function called after code reload:

```bash
calcit --reload-fn app.main/on-reload!
```

**--reload-libs**: Force reload library data during hot reload (normally cached):

```bash
calcit --reload-libs
```

### Config Entry (--entry)

Use a specific entry from `calcit.cirru`. Without this option Calcit selects `entries.default`; the selected entry's `:mode` chooses native execution or JS emission:

```bash
calcit --entry test
calcit --entry production
```

```cirru.no-check
:entries $ {}
  :default $ {} (:mode :js) (:init-fn 'app.main/main!) (:reload-fn 'app.main/reload!)
  :test $ {} (:mode :native) (:init-fn 'app.test/main!) (:reload-fn 'app.test/reload!)
```

The explicit `js` subcommand remains a compatibility/debug override. Prefer configuring `:mode` so the same entry is invoked consistently by developers and CI.

### Asset Watching (--watch-dir)

Watch additional directories for changes (e.g., assets, styles):

```bash
calcit --watch-dir assets/
calcit --watch-dir styles/ --watch-dir images/
```

## Common Usage Patterns

```bash
# Development with watch mode
calcit -w --reload-fn app.main/reload!

# Production build
calcit js --emit-path dist/

# JS watch mode
calcit js -w --emit-path dist/

# Testing single run
calcit --init-fn app.test/run-tests!

# Debug mode with full stack traces
calcit --reload-libs

# CI/CD environment
calcit --disable-stack
```

`calcit ir` 仅用于编译器内部表示与生成结果调试，不是普通应用的运行目标或 CI 完成证明。只有排查该层问题时才查看 `calcit ir --help`；日常项目验证仍使用直接运行、`--check-only`、`test` 或目标明确的 `js` / `wasm` / `wasi`。

## Snapshot 语义差异

升级 review 统一使用现有 `analyze program-diff` 入口，不再增加独立的 diff/query 命令：

```bash
# 工作区相对 HEAD 的人类可读 Markdown/tree 报告
calcit calcit.cirru analyze program-diff HEAD

# 自动化默认选择 Cirru EDN；JSON 只作为显式互操作格式
calcit calcit.cirru analyze program-diff HEAD --format edn
calcit calcit.cirru analyze program-diff HEAD --format json

# 比较两个 Git revision
calcit calcit.cirru analyze program-diff main --base v0.15.5 --format edn
```

结构化结果包含 Snapshot path、before/after content revision、稳定分类、受影响 definition 与是否需要 semantic review。`canonical-format-only` 表示原始文件字节不同、但解析后的 Snapshot 没有配置、schema、definition 或可执行表达式变化；它只能减少文本 diff 的人工确认，不能替代 strict check、测试或运行时验证。`--def` 保留原有的交互式 tree diff，目前只支持 human 输出；自动化证据应省略 `--def`，在同一份 Snapshot 报告中筛选 `:changes`。

## WASM preview 命令

`calcit wasm` 生成面向 browser/embedded host 的 core module；`calcit wasi` 默认仍生成可由 Wasmtime 等 WASI host 启动的 WASI Preview 1 command core module。显式使用 `calcit wasi --boundary component` 则生成 WASI 0.3.1 `wasi:cli/command` Component，目前支持零参数入口、`get-args`、`get-env`、`println` / `eprintln` / `echo` 和 `quit!` 的整数退出码；正常返回时退出码为 0。通用的 `calcit wasm --boundary component` 输出供 Component tooling 包装的 core module，不等价于 WASI 0.3.1 command。两个命令把 Snapshot 路径放在子命令之后，并分别通过 help 暴露输出契约：

```bash
calcit wasm calcit.cirru --emit-path js-out
calcit wasm calcit.cirru --boundary component --emit-path target/component-core
calcit wasi calcit.cirru --emit-path target/wasi-command
calcit wasi tests/fixtures/wasi-command-03.cirru --boundary component --emit-path target/wasi-03-command
wasmtime run -S p3 -W component-model-more-async-builtins=y -W component-model-async-stackful=y target/wasi-03-command/program.wasm
```

两个命令都支持 `--entry`、`--init-fn`、`--reload-fn` 和 `--check-only`。WASI 0.3 Component 的 `--check-only` 会完成 codegen 与封装验证，但不会写出 `program.wasm`。WASI command 的 init definition 必须为零参数；Component 路径还要求显式 `Unit` 返回 schema，避免把 `Result` 等返回值悄然当成成功退出。尚未迁移的 Preview 1 宿主能力在 Component 路径以 `E_WASI_COMMAND_CAPABILITY` 明确失败；编译器还会沿入口的直接调用链检查失败的依赖，不会把核心包装函数变成运行时 trap。当前无法证明目标安全的间接调用会以 `E_WASI_COMMAND_INDIRECT` 拒绝。通用 `defwasm-export` 以 `E_WASI_COMMAND_EXPORT` 拒绝，不会悄然丢弃。使用标准输出或标准错误的 Component 在 Wasmtime 49 中需要上述两个 `-W` 选项；未使用标准流的纯计算、参数和环境变量命令仍可只用 `-S p3`。标准输入、文件系统、时钟和随机数仍待后续 lowering。

WASI command 中的 `try-parse-cirru-edn-as` 与 `format-cirru-edn` 直接使用编译器已经推导出的闭合类型，不在运行时探测值类型。当前支持标量、递归 `List<T>`、标量 key 的 `Map<K,V>`，以及闭合 Struct field 和 Enum payload；core `Option<T>` / `Result<T,E>` 复用相同的 nominal Enum 路径。可完整往返的标量为 `Nil`、`Bool`、`String`、`Tag`、`Int8`、`UInt8`、`Int16`、`UInt16`、`Int32`、`UInt32`、`Int64` 与 `UInt64`。typed parser 还可读取 `Number`、`Float32` 与 `Float64`，但 formatter 尚不能为这些运行时浮点类型生成与 native 一致的文本，因此它们不属于当前支持的往返字段类型。Struct 输入使用 `%{} 'TypeName (:field value)`，Enum 输入使用 `%:: 'TypeName 'variant payload...`；名称、字段或 variant、payload 数量与递归 shape 都必须与声明完全一致。重复、缺失、未知项和数值越界都会返回 `Result :err`。格式化按声明字段或 variant 顺序生成 canonical Cirru EDN；开放 `Dynamic`、anonymous Enum 和其他未支持类型在 codegen 阶段明确拒绝，不会生成近似数据。

`calcit wasm --boundary component` 复用同一 WASM 入口，只输出供 Component tooling 包装的 core module，
不新增 component/WIT 顶层命令；WIT 生成与 runnable Component packaging 由独立的 `calcit-bindgen` 负责。
同步/异步类型闭包、流 adapter、Canonical ABI 限制与当前验证状态统一见
[WASM Component 边界](../installation/wasm-component-boundary.md)，此处不维护第二份能力清单。

WASM 可根据已解析的静态 callee 与函数参数 schema，特化携带非逃逸 inline closure 的普通函数调用；闭包在创建位置捕获词法局部值，因此 `Option.map`、`Result.map` 等静态方法不需要各自的 backend 拦截规则。动态 callee、可变参数函数、闭包逃逸与递归特化仍以 `E_WASM_CLOSURE_SPECIALIZATION` 明确失败，不会生成 `nil`、`0` 或失去捕获环境的替代实现。

WASI command 继续使用与原生、JavaScript 相同的 `get-env` 和 `get-args` API。默认 Preview 1 路径两者均可用；WASI 0.3 Component 的 `get-args` 通过 `wasi:cli/environment@0.3.1#get-arguments` 返回包含第 0 项的完整 `List<String>`。`get-env` 从同一接口的 `get-environment` 按名称查找，保留 `Option<String>`：未设置时为 `%none`，已设置为空字符串时为 `%some |`；非 ASCII 名称和值按 UTF-8 字节精确比较和复制。宿主只会提供显式授权的环境变量，例如用 `wasmtime run -S p3 --env CALCIT_TEST=你好 program.wasm`。Preview 1 与 Component 的内存 ABI 都只存在于编译器内部，不进入 Calcit 源码接口。

`println` 和 `echo` 写标准输出，`eprintln` 写标准错误，保留参数间空格与结尾换行。编译器通过 `wasi:cli/stdout` / `stderr@0.3.1` 的字节流处理部分写入，支持 UTF-8 和长文本；标准输入尚未支持。输出流关闭后会等待宿主 completion future 并检查结果，失败时陷阱终止，而不会误报成功。`println` 等表层函数仍不返回 `Result`，因此不要把它们当成可靠持久化或审计通道。

command init definition 正常返回时，进程状态为 `0`；调用 `quit!` 可显式设置 `0..255` 的整数退出状态。Preview 1 路径在内部调用 `proc_exit`，WASI 0.3 Component 路径调用 `wasi:cli/exit@0.3.1#exit-with-code`；Calcit 源码无需感知这两套 ABI。

版本坐标（2026-09-23）：WASI 规范 release 为 `v0.3.1`，这里使用的 `wasi:cli`、`wasi:clocks`、`wasi:filesystem`、`wasi:random` 和 `wasi:sockets` WIT package 均为 `0.3.1`，来源与校验值由 `calcit-bindgen 0.1.9` 维护；其 Component 包装依赖 `wit-component 0.258.0`。实际运行验收使用已发布的 Wasmtime `49.0.0`；未调用标准流的命令形如 `wasmtime run -S p3 program.wasm`，调用标准流的命令还需要前述两个 async 特性开关。Wasmtime 49 内置的 p3 CLI WIT 仍标记为 `0.3.0`，但生成的 `0.3.1` Component 已用正式二进制测试参数、环境、标准输出/标准错误、退出码及拒绝路径；不能仅凭内置文件的版本文字判断兼容性，也不能把 WIT parse 当作运行验收。默认目标仍是 Preview 1，切换由 #1269 负责。

WASI command 也复用 `unix-time-ms` 与 `cpu-time`。前者读取系统实时时钟；后者读取单调时钟，只保证同一进程内两次读数的差值有意义。两者均返回毫秒数；Preview 1 的纳秒结果与错误码由编译器内部转换和检查，宿主失败时不会返回 `0` 或 `nil`。

同步等待使用 `wait-ms`，参数必须是 `0..4294967295` 范围内的整数毫秒，返回 `Result<Unit,String>`。零值立即成功且不调用 host；WASI command 把非零等待转换为 Preview 1 `poll_oneoff` 的相对单调时钟订阅，并验证返回事件。小数、负数、溢出和宿主错误都进入 `Result :err`，不做舍入或伪造成功。`calcit wasm` 的 core module 没有默认阻塞等待协议，会以 `E_WASM_CAPABILITY` 拒绝。该 API 不改变 JavaScript callback 形式的 `timeout-call`，也不等同于 fire-and-forget 的 `async-sleep`；由于控制流、单位和生命周期都不同，自动 fix 不应改写这两个旧入口，只能提示用户选择同步 `wait-ms` 或异步 callback/task API。

安全随机字节统一通过 `secure-random-bytes` 获取。参数必须是 `0..65536` 范围内的整数，返回值为 `Result<Buffer,String>`；WASI command 由 Preview 1 `random_get` 填充缓冲区，宿主错误保留为 `Result` 的错误分支。Native 与生成的 JavaScript 保持相同的公开值形状，分别使用系统 CSPRNG 与 Web Crypto。`calcit wasm` 的 core module 没有默认随机数宿主协议，会以 `E_WASM_CAPABILITY` 明确拒绝。

WASI command 可通过 `FsPath .read-text` 与 `.write-text` 访问 host 显式预开放的目录。Calcit 路径使用 guest 侧名称，例如 host 以 `--dir ./data::/workspace` 授权后，程序访问 `workspace/input.txt`；编译器会选择最长匹配的 preopen，并只把剩余相对路径交给 Preview 1 `path_open`：

```bash
calcit wasi calcit.cirru --emit-path target/wasi-command
wasmtime run --dir ./data::/workspace target/wasi-command/program.wasm
```

仓库中的 [`examples/wasi-command/`](../../examples/wasi-command/) 提供了一个可直接运行的文本处理项目：它从命令行接收 guest 输入/输出路径，通过环境变量选择前缀，处理 Result 错误分支，并以稳定的非零状态码报告参数、读取或写入失败。新项目应先复用这个单一 `calcit wasi` 工作流，不需要增加包装命令。

未提供 preopen、以 `/` 开头的绝对路径、包含完整 `..` 分段的越界路径、非法 UTF-8、I/O 错误和无法继续推进的 partial I/O 都返回 `Result :err`。当前 WASI 文本读取单文件上限为 4 MiB。`.read-dir` 枚举即时子项，沿 Preview 1 cookie 处理分页和截断记录，过滤 `.`、`..` 后按完整 guest path 排序；单次最多返回 4096 项，累计路径字节最多 4 MiB，单个 UTF-8 名称最多 4096 字节。超过限制同样返回错误，避免模块为不受控输入分配过量线性内存。Calcit 不接触 raw descriptor；`calcit wasm` 的 core module 也不会继承文件权限，而是在 codegen 阶段以 `E_WASM_CAPABILITY` 拒绝。递归 `.walk-dir` 尚未接入 WASI。

## Markdown 代码块检查

使用 `docs check-md` 验证 Markdown 文件中的 Cirru 代码块：

```bash
calcit docs check-md README.md
```

默认读取 `calcit.cirru` 作为求值 Snapshot。项目使用其他文件名时，以 `--snapshot` 指定文件路径；`--entry` 在其他常用命令中表示 Snapshot 内的 named entry，不再用于选择文件：

```bash
calcit docs check-md README.md --snapshot calcit.cirru
```

需要额外模块时可重复传入 `--dep`：

```bash
calcit docs check-md README.md --dep ./ --dep ~/.config/calcit/modules/memof/
```

Format the same fenced Cirru blocks with `docs format-md`. It preserves all
Markdown outside recognized `cirru`, `cirru.no-run`, `cirru.no-check`,
`cirru.cli`, and `cirru.edn` fences, and writes through an atomic replacement:

```bash
calcit docs format-md README.md
```

Use `--check` in CI to reject non-canonical snippets without changing files:

```bash
calcit docs format-md README.md --check
```

Recommended block modes:

- `cirru`: run + preprocess + parse (preferred; executes injected snippet entry `app.main/main!`, not entry file `:init-fn`)
- `cirru.no-run`: preprocess + parse when runtime setup is unavailable
- `cirru.no-check`: parse only for illustrative snippets
- `cirru.edn`: not Calcit code — parse as EDN data, for schema/config snippets such as `CodeEntry :schema`/`:ffi` payloads

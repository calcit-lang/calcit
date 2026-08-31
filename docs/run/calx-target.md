# 实验性 Calx target：typed scalar kernel lowering

Calcit 正在验证一个很窄的 typed scalar kernel 子集能否编译到 Calx。这个实验不改变默认 native
runner、JS codegen 或仓库内部 WASM backend，也不会把任意 Calcit 函数静默发送给另一个运行时。

当前阶段建立 eligibility boundary，并把通过证明的 closed kernel 降为可执行的 strict Calx program：

```text
explicit namespace/definition
  -> macro-expanded, symbol-resolved, typed CompiledProgram snapshot
  -> closed reachable direct-call graph
       -> structured FallbackReport
       -> typed lowering plan
       -> explicit signature-matched host capabilities
       -> ProgramBuilder -> CalxProgram -> ValidatedProgram
       -> CalxVM::run_typed
```

Rust embedding 可对同一个不可变 snapshot 调用：

```rust
use calcit::calcit::Calcit;
use calcit::codegen::calx::{analyze_calx_eligibility, compile_calx_kernel};

let snapshot = calcit::program::clone_compiled_program_snapshot()?;
match analyze_calx_eligibility(&snapshot, "app.kernel", "range-sum") {
  Ok(graph) => println!("{}", graph.stable_summary()),
  Err(report) => eprintln!("{}", report.stable_summary()),
}

let kernel = compile_calx_kernel(&snapshot, "app.kernel", "range-sum")?;
let value = kernel.run(&[Calcit::Number(10.0), Calcit::Number(0.0)])?;
```

需要跨重复请求复用完整 validated artifact 时，由 embedding 显式持有有界 cache；cache 不进入全局状态：

```rust
use calcit::codegen::calx::{CalxCompileCache, CalxHostImports};

let mut cache = CalxCompileCache::new(8);
let imports = CalxHostImports::new();
let preparation = cache.prepare(&snapshot, "app.kernel", "range-sum", &imports)?;
assert!(!preparation.report().cache_hit);

let hit = cache.prepare(&snapshot, "app.kernel", "range-sum", &imports)?;
assert!(hit.report().cache_hit);
let value = hit.kernel().run(&[Calcit::Number(10.0), Calcit::Number(0.0)])?;
```

cache 保存的 `CalxCompiledArtifact` 只有 eligible graph、strict boundary、`ValidatedProgram`、reachable
definition structural stamps、used-import schema stamps、ABI 与 callback-free import contract。`CalxPreparedKernel` 每次由当前
`CalxHostImports` 新建，host callback、capability state、VM、buffer input 与 runtime state 都不会进入
artifact。命中逐项核对旧 reachable set；entry/direct/transitive/schema 变化会 miss，无关 definition
变化保持 hit。API、LRU、eviction ledger、miss reason 与统计契约见
[revision-safe cache 文档](./calx-compile-cache.md)。

需要宿主能力时，embedding 必须显式构造 `CalxHostImports`，以 Calcit definition 为 capability key，
再调用 `analyze_calx_eligibility_with_imports` / `compile_calx_kernel_with_imports`。普通未知调用不会自动
变成 import，allowlist 中的 declaration 也必须与 typed snapshot 的 fixed-arity Number/Bool/Unit
签名完全一致，否则整个 kernel 在 lowering 前结构化回退。

`analyze_calx_eligibility` 不执行代码，也不产生半个 Calx program。只有入口可达的每个 definition
都通过时才返回 `CalxEligibleCallGraph`；任意 callee 不合格都会返回覆盖整个 closure 的
`CalxFallbackReport`。

`compile_calx_kernel` 只在完整 graph eligible 后创建 lowering plan，再统一提交给 `ProgramBuilder` 并转换为
`ValidatedProgram`。entry 在 Calx 内命名为 `main`，其余 reachable functions 使用确定的 fully-qualified
name；direct tail call 与 `recur` 降为 `return-call`。

运行边界严格按已证明签名逐项转换：Calcit `Number` 对应 Calx `F64`，Calcit `Bool` 对应 Calx
`Bool`，void result 对应 Calcit `Unit`。`Nil`、Dynamic 或任意不匹配的 runtime value 会在进入 VM
之前被拒绝，不会被编码为占位值。

## 首批接受范围

- function boundary 与 local：`Number`、`Bool`；函数结果额外接受 `Unit`，映射为 void；
- fixed arity top-level function；
- Number/Bool literal、typed local、单 binding `&let`、有 else 的 `if`；
- `&+`、一元/二元 `&-`、`&*`、`&/`、`&=`、`&<`、`&>`；
- fixed-arity direct call 与 tail-position `recur`；
- 显式 allowlist 的 zero-result / single-result typed host import；
- 条件必须静态为 Bool，不复用 Calcit 或 Calx 的 numeric truthiness。

首批明确拒绝：

- `Dynamic`、`Nil`、Optional/JsNullish，以及除 Number/Bool/Unit 之外的 boundary/storage type；
- closure、function value、local/dynamic operator、HOF、rest/optional arity；
- 无 else 的 `if`、非 tail `recur`、global/ref/atom、collection/nominal value；
- 未加入 allowlist 的 host/native capability。

## Typed host import contract

`CalxHostImport::void` 绑定 `Result<(), CalxError>`，`CalxHostImport::value` 绑定
`Result<CalxValue, CalxError>`。参数由 VM 在 callback 前按 F64/Bool 声明检查，single-result callback
返回的 owned value 也会再次检查。Nil、Dynamic、多个结果和隐式类型转换都不进入该 ABI。

每次 kernel `run` 都创建独立 VM instance，并复用编译期固定的函数指针 binding；参数以 owned Calx
scalar 进入 VM，callback 只借用本次调用的 slice，single result 由 callback 转移给 VM。embedding
负责 capability 的外部状态与并发策略。callback 一旦执行，无论成功或 trap 都不会自动回退到 Calcit，
因此 effect 不会因双执行而重复。

## Fallback contract

fallback 使用 ABI edition `calcit-calx-kernel/1`，并保留 entry、失败 definition、可用的 source tree
path、call path、稳定 code 与人类可读 message。稳定 code 包括：

- `CALX_SUBSET_DYNAMIC_TYPE`
- `CALX_SUBSET_NIL_VALUE`
- `CALX_SUBSET_UNSUPPORTED_TYPE`
- `CALX_SUBSET_UNSUPPORTED_FORM`
- `CALX_SUBSET_INDIRECT_CALL`
- `CALX_SUBSET_ARITY`
- `CALX_SUBSET_NON_BOOL_CONDITION`
- `CALX_SUBSET_RECUR_NOT_TAIL`
- `CALX_SUBSET_HOST_CAPABILITY`
- `CALX_SUBSET_CALL_CLOSURE`
- `CALX_SUBSET_ABI_EDITION`

issues 按 code、definition、source path 与 call path 确定排序。`stable_summary()` 用于 repository
golden tests，但明确是 experimental report，不是可持久化的 compiler ABI。

## Source-backed fixtures

[`tests/fixtures/calx/scalar-kernels.cirru`](../../tests/fixtures/calx/scalar-kernels.cirru)
包含五类真实 Calcit source：

- `range-sum`：Number comparison 与 tail recur；
- `fibonacci`：if、F64 comparison 与 direct recursion；
- `affine`：多参数算术和 direct helper call graph；
- `polynomial`：固定深度数值表达式；
- `bounded-simulation`：随输入规模增长的数值状态迭代。

测试先让这些 definitions 经过 Calcit preprocessing，再从 `CompiledProgram` snapshot 分析和 lowering；
对应 golden 固定 ABI、entry、函数签名、确定排序与 direct-call edges。五个 kernel 都会在 strict Calx
VM 中实际执行，并与同一源码的 Calcit native runner 做差分比较。另一个 fixture 固定 Dynamic callee
导致整个入口 closure fallback 的报告，并验证不会进入 lowering。

[`tests/fixtures/calx/typed-imports.cirru`](../../tests/fixtures/calx/typed-imports.cirru)
覆盖 zero-result observe capability、single-result numeric capability 和 trapping capability。对应
generated-program golden 固定 import declaration、guest syntax 与 Calcit tree origin；trap golden 固定
`CALX_HOST_IMPORT` 诊断。签名不一致的显式 capability 在 lowering 前整体 fallback。

## 错误与回退边界

- `Eligibility` 是唯一允许 embedding 选择整体回退到 Calcit 的编译结果；
- `Lowering` 表示 eligibility 与 lowering 对 typed expression 的理解不一致；
- `Build` / `Validation` 表示生成程序违反 Calx contract；
- `Instantiate` / `Runtime` 表示 binding 或执行失败。

后四类不是 eligibility fallback，运行失败后也不会自动重跑 Calcit，以免未来 effect import 被执行两次。

## 尚未实现

当前 API 仍是 Rust embedding，不是 `calcit` CLI 的正式 backend。首批没有 collection/nominal value、closure、
持久化 cache、VM pool 或 selection policy，也还没有基于 benchmark 的自动 offload。embedding-owned、
容量有界、revision-safe 的 validated-artifact cache 已实现，但 cache-hit 性能报告仍等待独立 harness。
correctness corpus 已覆盖 scalar
kernel、zero/single-result typed imports、generated program、trap 与 fallback。分阶段 benchmark matrix、
采样 crossover point 和首份 scalar baseline 已建立；[calx-vm #39](https://github.com/calcit-lang/calx-vm/issues/39)
现已补充公平的 cached Calcit callable 基线：有限 scalar 样本仍显示 Calx hot 收益，但 lookup-call 对比确实
夸大了微型 kernel 差距。[compile profile](../../benchmarks/calx/20260831-compile-profile-macos-arm64.json)
进一步确认 program construction 及其 expression/source-origin emission 是首要 CPU 与分配目标；
[revision-safe cache design](./calx-compile-cache.md) 因此选择缓存完整 validated source-derived artifact，并在
每次命中重新附着 host bindings。VM pooling 不在缺少独立证据时提前实现；实用 bulk workload 由
[calx-vm #50](https://github.com/calcit-lang/calx-vm/issues/50) 以严格、
non-nil、zero-Dynamic typed buffer 单独推进，再决定 selection policy。
基准命令、阶段定义和原始 JSON 格式见[《Calcit→Calx benchmark methodology》](./calx-benchmark.md)。

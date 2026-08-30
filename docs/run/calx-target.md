# 实验性 Calx target：eligibility 与整体回退

Calcit 正在验证一个很窄的 typed scalar kernel 子集能否编译到 Calx。这个实验不改变默认 native
runner、JS codegen 或仓库内部 WASM backend，也不会把任意 Calcit 函数静默发送给另一个运行时。

当前第一阶段只建立编译前的 eligibility boundary：

```text
explicit namespace/definition
  -> macro-expanded, symbol-resolved, typed CompiledProgram snapshot
  -> closed reachable direct-call graph
       -> eligible graph
       -> structured FallbackReport
```

Rust embedding 可对同一个不可变 snapshot 调用：

```rust
use calcit::codegen::calx::analyze_calx_eligibility;

let snapshot = calcit::program::clone_compiled_program_snapshot()?;
match analyze_calx_eligibility(&snapshot, "app.kernel", "range-sum") {
  Ok(graph) => println!("{}", graph.stable_summary()),
  Err(report) => eprintln!("{}", report.stable_summary()),
}
```

`analyze_calx_eligibility` 不执行代码，也不产生半个 Calx program。只有入口可达的每个 definition
都通过时才返回 `CalxEligibleCallGraph`；任意 callee 不合格都会返回覆盖整个 closure 的
`CalxFallbackReport`。

## 首批接受范围

- function boundary 与 local：`Number`、`Bool`；函数结果额外接受 `Unit`，映射为 void；
- fixed arity top-level function；
- Number/Bool literal、typed local、单 binding `&let`、有 else 的 `if`；
- `&+`、一元/二元 `&-`、`&*`、`&/`、`&=`、`&<`、`&>`；
- fixed-arity direct call 与 tail-position `recur`；
- 条件必须静态为 Bool，不复用 Calcit 或 Calx 的 numeric truthiness。

首批明确拒绝：

- `Dynamic`、`Nil`、Optional/JsNullish，以及除 Number/Bool/Unit 之外的 boundary/storage type；
- closure、function value、local/dynamic operator、HOF、rest/optional arity；
- 无 else 的 `if`、非 tail `recur`、global/ref/atom、collection/nominal value；
- 未加入 allowlist 的 host/native capability。

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
包含首批三类真实 Calcit source：

- `range-sum`：Number comparison 与 tail recur；
- `fibonacci`：if、F64 comparison 与 direct recursion；
- `affine`：多参数算术和 direct helper call graph。

测试先让这些 definitions 经过 Calcit preprocessing，再从 `CompiledProgram` snapshot 分析；对应
golden 固定 ABI、entry、函数签名、确定排序与 direct-call edges。另一个 fixture 固定 Dynamic
callee 导致整个入口 closure fallback 的报告。

## 尚未实现

本阶段故意不依赖未发布的 `calx_vm` git revision，也不复制 Calx IR。后续阶段会在包含
`ProgramBuilder` 的稳定 calx_vm crate 可用后，把 eligible graph 和同一份 typed expressions 降为：

```text
ProgramBuilder -> CalxProgram -> ValidatedProgram -> CalxVM::run_typed
```

build/validation/binding failure 与 runtime trap 不属于 eligibility fallback，运行失败后也不会自动
重跑 Calcit，以免 effect import 被执行两次。

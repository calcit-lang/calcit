# Calx benchmark session adapter

## 中文

### 背景

独立 `calcit-calx-bench` 已建立，但过渡 runner 仍直接访问
`PROGRAM_CODE_DATA`、`ProgramFileData`、`ensure_def_id`、`run_fn` 等进程级内部实现。
这会把 benchmark 产品绑定到 mutable compiler registry，也无法形成 #557 约定的
revision-pinned session boundary。

### 本次改动

- 增加 doc-hidden 的 `codegen::calx::benchmark` 内部模块；
- 通过显式命名 corpus 和完整 concrete scalar schema 建立单活动 session；
- session 一次完成 source install、preprocess 和 immutable `CompiledProgram` snapshot；
- 封装 cached Calcit callable、measured Calx compile、revision-safe cache prepare、strict
  argument/result conversion 和稳定 program counts；
- 使用显式 `Unit` / scalar return contract，不允许 Nil、Dynamic 或 persistent collection
  coercion；
- 过渡 runner 改为只消费 session adapter，并增加 source-level forbidden-symbol regression；
- 更新 extraction manifest，记录独立仓库已确认和 adapter implementation path。

### 约束与经验

现有 Calcit preprocess/runtime 仍使用进程级 registry，cached callable 的同命名空间调用也可能
读取 runtime cell。因此 adapter 串行持有 session guard，并在 session 生命周期内保留已安装
namespace；它是固定 revision 的 benchmark API，不是并发 embedding API，也不承诺 semver。
benchmark 采样、迭代次数、统计和 fallback policy 继续留在独立 harness。

## English

### Context

The standalone `calcit-calx-bench` repository existed, but the transitional
runner still reached process-level compiler internals directly. That coupled
the product to mutable registries and did not satisfy the revision-pinned
session boundary frozen by #557.

### Changes

- Add the doc-hidden `codegen::calx::benchmark` internal module.
- Build one serialized session from an explicitly named corpus and a complete
  concrete scalar schema table.
- Perform source installation, preprocessing, and immutable snapshot creation
  once per session.
- Encapsulate cached Calcit calls, measured Calx compilation, revision-safe
  cache preparation, strict value conversion, and stable program counts.
- Model Unit explicitly and admit no Nil, Dynamic, or persistent-collection
  coercion.
- Refactor the transitional runner to consume only the adapter and guard that
  boundary with a forbidden-symbol contract regression.
- Update the extraction manifest with the confirmed repository and adapter
  implementation path.

### Constraint

Calcit preprocessing and runtime resolution remain process-global today, and
same-namespace calls from cached functions may consult runtime cells. The
adapter therefore serializes active sessions and retains the installed
namespace for the session lifetime. It is a revision-pinned benchmark API, not
a concurrent embedding API or a semver-stable surface. Sampling, statistics,
and fallback policy stay in the standalone harness.

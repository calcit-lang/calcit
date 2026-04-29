# Fix: bind-type 类型槽预编译顺序问题

## 问题描述

`cr js` 在 tiye-index 项目中，`reel.app.comp.todolist.mjs` 和 `respo.schema.mjs` 仍然不稳定——有时生成 `Op`-aware 版本（带 `import { Op }` 和 `$clt._PCT__$o__$o_(Op, ...)`），有时生成不带 Op 的版本（使用 `$clt._$o__$o_(...)`）。

## 根本原因

`bind-type :dispatch-op Op` 在 `respo.main/main!` 函数体中，是一个预处理阶段的副作用操作：当 `preprocess_expr` 遇到 `bind-type` 调用时，立即执行 `bind_type_slot("dispatch-op", Op)`，使 `*dispatch-op` 类型槽指向 Op 枚举。

问题根源：`reel.app.comp.todolist` 等组件 def 会作为入口函数 `app.main/main!` 的传递依赖被提前编译。由于 HashMap 迭代顺序不确定，某些运行中 `todolist` 在 `respo.main/main!` 之前编译——此时 `*dispatch-op` 类型槽尚未绑定，导致 `d! $ :: :states cursor ...` 中的元组字面量无法被改写为 `%:: Op :states cursor ...`，生成结构上不同的 JS 代码。

## 修复方法

**双重修复：**

### 1. 预处理阶段：`precompile_bind_type_defs` (主要修复)

在 `src/lib.rs` 的 `run_program_with_docs` 中，在开始主入口预处理之前，先扫描**所有**程序源代码，找出含有 `bind-type` 调用的 def，并提前编译它们。

这确保 `respo.main/main!`（含 `bind-type :dispatch-op Op`）在 `app.main/main!` 的依赖树遍历之前被编译，从而使 `:dispatch-op` 类型槽在所有组件编译前就已绑定。

### 2. 快照填充阶段：任务排序 (辅助修复)

在 `src/program.rs` 的 `collect_snapshot_fill_tasks` 中，对快照填充任务排序，确保含 `bind-type` 的 def 在快照阶段也优先处理，同时按 (ns, def) 字典序排序保证完全确定性。

## 已修改文件

- `src/lib.rs`: 在 `run_program_with_docs` 中调用新增的 `precompile_bind_type_defs`
- `src/runner/preprocess/mod.rs`: 新增 `precompile_bind_type_defs` 公开函数（扫描所有源 def，提前编译含 `bind-type` 的 def）
- `src/program.rs`:
  - `calcit_contains_bind_type` 改为 `pub fn`（供 preprocess 模块使用）
  - `collect_snapshot_fill_tasks` 中对任务排序（bind-type 优先，其余按字典序）

## 验证

- `cr js` 在 tiye-index 项目连续 8 次运行，第 1 次（旧输出重新生成）后无任何文件变化
- `cargo test --lib` 179 tests passed

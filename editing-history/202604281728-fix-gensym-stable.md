# Fix: Gensym 稳定性问题

## 问题描述

`cr js` 每次运行会生成不同的 JS 文件，原因是 gensym 计数器不稳定（如 `v__1` 变成 `v__3`）。

## 根本原因

Calcit 的 gensym 使用 `NS_SYMBOL_DICT: HashMap<Arc<str>, usize>` 记录计数器。由于 Rust 的 `HashMap` 每进程随机化迭代顺序，不同运行时的 def 编译顺序不同，导致 gensym 计数器累积值不一样。

有**两条**编译路径，只修了一条是不够的：
1. `ensure_ns_def_preprocessed` —— 正常编译路径，通过 `with_compiling_def` 重置 gensym 计数器（之前已修）
2. `compile_source_def_for_snapshot` —— 快照填充路径（JS codegen 时触发），**缺少 `with_compiling_def` 包装**，导致 `CURRENT_COMPILING_DEF` 为 `None`，gensym 退回到 `file_ns`（命名空间级 key），计数器跨 def 累积

## 修复方法

在 `src/runner/preprocess/mod.rs` 的 `compile_source_def_for_snapshot` 函数中，为 `preprocess_expr` 调用加上 `builtins::meta::with_compiling_def(ns, def, ...)` 包装，与 `ensure_ns_def_preprocessed` 保持一致。

## 已修改文件

- `src/builtins/syntax.rs`: gensym key 改为 `CURRENT_COMPILING_DEF`（无 `file_ns` 前缀）
- `src/builtins/meta.rs`: 新增 `CURRENT_COMPILING_DEF` thread_local + `with_compiling_def`（自动重置计数器）
- `src/runner/preprocess/mod.rs`:
  - `ensure_ns_def_preprocessed`: 包 `with_compiling_def`（第一次修）
  - `compile_source_def_for_snapshot`: 包 `with_compiling_def`（本次修）

## 验证

- `cr js` 连续 8 次运行，无任何文件变化
- `cargo test` 67 passed
- `yarn check-all` 全部通过

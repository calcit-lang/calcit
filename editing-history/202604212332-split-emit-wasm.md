# 202604212332 - Split emit_wasm.rs into focused submodules

## 背景

`src/codegen/emit_wasm.rs` 原来有 6353 行，包含 WASM codegen 的所有逻辑，难以维护。

## 拆分方案

将文件按数据结构类别拆分为 6 个新子模块（位于 `src/codegen/emit_wasm/`）：

| 文件 | 行数 | 内容 |
|------|------|------|
| `heap.rs` | 266 | 内存管理：bump allocator、类型标签查找、哈希辅助函数 |
| `lists.rs` | 1130 | 列表操作 + BufList + `emit_range` + `emit_list_distinct` |
| `maps.rs` | 1157 | Map 基础操作 + merge/diff + `&map:destruct` + `&merge-non-nil` |
| `sets.rs` | 828 | Set 操作 + `&set:destruct` |
| `strings.rs` | 925 | 字符串操作 + pad + `build_str_*` runtime 函数 |
| `hof.rs` | 423 | `resolve_callee_fn_idx`、`foldl`、`foldl-shortcut`、`foldr-shortcut` |

父文件 `emit_wasm.rs` 精简到 1656 行，保留：
- 模块级 doc、import、常量、基础 helpers
- `emit_wasm()` 公开入口函数
- 所有结构体和 impl（`CompiledFn`、`WasmCompileEnv`、`WasmGenCtx`）
- 编译管道函数（`extract_fn_parts`、`compile_fn`、`emit_body`）
- 表达式 emitter（`emit_expr`、`emit_call_expr`、`emit_proc_call` 等）
- Tag/String pool 收集

## 模块间可见性

- 各子模块顶部均有 `use super::*;`（继承父模块 imports）
- 所有导出函数使用 `pub(super) fn`（通过 Python 正则批量转换）
- 父模块通过 `pub(super) use heap::*` 将 heap 辅助函数传播给兄弟模块
- 父模块通过 `use lists::*; use maps::*; ...` 将各域函数引入 `emit_proc_call` 的作用域

## 实现技术

使用 Python 脚本（`scripts/split_emit_wasm.py`）自动化提取：
- 精确定位每段的起止行（避免孤立的 doc comment）
- 对非连续段（如 maps.rs 包含 4 段原文）进行拼接
- `make_pub_super()` 批量将 `^fn ` 替换为 `pub(super) fn`

## 验证

- `cargo check`：无错误，仅 1 个 `pub(super) use heap::*` 可见性警告（已用 `#[allow(unused_imports)]` 消除）
- `./target/release/cr calcit/test-record.cirru`：测试全部通过

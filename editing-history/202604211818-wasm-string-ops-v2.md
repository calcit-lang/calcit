# WASM String Ops 扩展 v2

## 本次修改概要

继上一轮字符串实现（count/nth/first/rest/slice/concat/compare）之后，补充剩余实用 API。

## 新增 API

| Proc | 实现位置 | 说明 |
|------|----------|------|
| `&str:contains?` | `emit_str_contains` | 字节索引范围检查：`byte_len > idx` |
| `&str:find-index` | `__rt_str_find_index` (runtime.rs) | 朴素 O(n·m) 字节子串搜索，返回偏移或 -1 |
| `&str:includes?` | `emit_str_includes` | 调用 `find-index`，判断 `>= 0` |
| `&str:pad-left` | `__rt_str_pad_left` (emit_wasm.rs) | 循环填充 pattern 字节于左侧 |
| `&str:pad-right` | `__rt_str_pad_right` (emit_wasm.rs) | 循环填充 pattern 字节于右侧 |

## 关键实现细节

### `__rt_str_find_index`

- 放在 `runtime.rs`，不需要 `str_tag_id`（只读不分配）
- 使用双层嵌套 Loop/Block 结构，WASM 控制流通过 `Br(N)` 跳出多层
- 空 needle 返回 0.0；needle 长于 haystack 返回 -1.0
- `Br(5)` 从内层 If 跳出到 `$outer` Block 返回找到的索引

### `__rt_str_pad_left` / `__rt_str_pad_right`

- 放在 `emit_wasm.rs`，接受 `str_tag_id: i32` 参数（需分配 tagged 堆内存）
- 早退条件：`str_len >= target_size` 直接返回原指针
- 使用 j 变量追踪 pattern 偏移，超出 pat_len 时归零，避免 modulo 除法
- 注册在 `build_runtime_fns` 之后（与 `__str_new` 同一阶段）

### `emit_str_contains` 修复

初始版本使用 `I32LtU`（`byte_len < idx`）导致逻辑反转，修正为 `I32GtU`（`byte_len > idx`）。

## 测试

新增 8 个测试函数（`test-wasm.main`）：
- `test-str-contains-true` → 1
- `test-str-contains-false` → 0
- `test-str-find-index-found` → 1（"ell" 在 "hello" 偏移 1）
- `test-str-find-index-not-found` → -1
- `test-str-includes-true` → 1
- `test-str-includes-false` → 0
- `test-str-pad-left` → 5（`pad-left "hi" 5 "-"` → "---hi"，count=5）
- `test-str-pad-right` → 5（`pad-right "hi" 5 "-"` → "hi---"，count=5）

`yarn try-wasm` 全部通过（release build）。

## 文件变更

- `src/codegen/emit_wasm/runtime.rs`：新增 `build_rt_str_find_index`，在 `build_runtime_fns` 中注册
- `src/codegen/emit_wasm.rs`：新增 `build_str_pad_left_fn`、`build_str_pad_right_fn`、`emit_str_contains`、`emit_str_find_index`、`emit_str_includes`、`emit_str_pad_left`、`emit_str_pad_right`；修复 contains 比较方向
- `calcit/test-wasm.cirru`：通过 `cr edit def` 添加 8 个测试定义
- `scripts/test-wasm.mjs`：新增 8 个 `check()` 调用
- `docs/wasm-codegen.md`：更新支持表格，移除已实现项

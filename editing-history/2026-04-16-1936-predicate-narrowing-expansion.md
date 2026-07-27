# 扩展 extract_predicate_binding 支持更多类型谓词

## 改动概要

将 `extract_predicate_binding` 重构为 `extract_predicate_bindings`，支持双向窄化（true/false 分支），并新增 6 个类型谓词的窄化支持。

## 修改文件

### src/runner/preprocess/mod.rs

- 重构 `extract_predicate_binding` → `extract_predicate_bindings`，返回 `PredicateNarrowing` 结构体（包含 `true_binding` 和 `false_binding`）
- `preprocess_if` 中 false 分支也应用窄化信息
- 新增谓词：`tag?` → `:tag`、`bool?` → `:bool`、`symbol?` → `:symbol`、`fn?` → `:fn`
- 新增 `nil?`/`some?`：当变量已知为 `Optional(T)` 时，nil? 的 false 分支窄化为 `T`，some? 的 true 分支窄化为 `T`

### src/calcit/proc_name.rs

- `TurnTag` 的参数签名从 `some_tag("string")` 改为 `dynamic_tag()`，因为 `turn-tag` 在运行时接受 tag/symbol/string，旧签名在 `tag?` 窄化后误报

## 知识点

1. **双向窄化**：`if (nil? x) ... else ...`，false 分支可从 `Optional(T)` 解包为 `T`，这在链式 if 模式中可以传播类型信息
2. **新窄化暴露的类型签名问题**：`tag?` 窄化后，`(turn-tag x)` 触发了 arg-type-mismatch 警告，因为 `turn-tag` 签名过严。类似 coercion 函数应使用 `dynamic_tag()`
3. **核心库中 `if (tag? ...)` 出现 5 次、`if (nil? ...)` 26 次、`if (list? ...)` 41 次**——窄化覆盖面广
4. **JS 端 tuple `.nth` 缺失**：prior session 简化了多态函数（移除 tuple? 分支），但 JS runtime 的 `invoke_method` 没有对应的 core-tuple-methods 注册，导致 JS 测试 `test_refs` 失败。这是 JS procs 层的问题，不是预处理阶段的问题

## 测试结果

- Rust: 246 tests pass（67 cirru + 179 unit）
- Clippy: clean
- JS: 已知 pre-existing failure（tuple .nth in JS runtime）

# 2026-03-04 16:45 `Calcit` 等值契约收敛、比较逻辑拆分与宏测试期望对齐

## 背景

在将 `Calcit` 的 `Eq/Hash/Ord` 契约对齐后，`Symbol/Local/Import` 的跨变种相等被移除，暴露了若干 `macroexpand-all` 测试中“符号 vs 已解析 core import”比较不一致的问题。

## 本次改动

- 运行时契约修复（`src/calcit.rs`）：
  - 移除 `Symbol/Local/Import` 跨变种相等特判。
  - 补齐 `Trait/Impl/Struct` 相关 hash 字段，避免 `Eq` 与 `Hash` 不一致。
  - `Struct/Enum/Trait/Impl` 的 `Ord` 比较从仅按 name 改为与 `Eq` 语义一致。
- 比较辅助函数拆分（`src/calcit/compare.rs`）：
  - 将 `compare_*` 辅助逻辑独立成模块，减轻 `calcit.rs` 体积与复杂度。
- 宏测试期望修复（`calcit/test-macro.cirru`）：
  - 将多处 `quasiquote` 直接期望改为“左右均 `macroexpand-all` 再比较”，避免因 `calcit.core/+`、`calcit.core/=` 的 `Import`/`Symbol` 差异导致伪失败。

## 经验与约束

- 当核心值语义从“宽松相等”收敛为“严格相等”后，宏展开测试应优先比较**同一归一化阶段**产物，而不是混比 `quasiquote` 与 `macroexpand-all` 原始结果。
- 若测试目标是“展开形状正确”，可比较两侧都经过 `macroexpand-all` 的 AST，降低名称解析层差异带来的噪声。

## 验证

- `cargo test`
- `yarn check-all`

均通过。

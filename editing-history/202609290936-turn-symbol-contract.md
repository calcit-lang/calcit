# `turn-symbol` 的成功值类型收敛

## 依据

`turn-symbol` 的 native 和 JS 实现对 String 输入都只返回 Symbol，内部 proc 签名也已经是 `String -> Symbol`，但 core Snapshot 的公开 schema 仍是 `Dynamic -> Dynamic`。这让 Agent 查询和静态调用方失去已有的类型证据。现将 schema 对齐为 `String -> Symbol`，不改运行时实现，也不在本批引入计划中的新名称。

native/JS 运行时历史上还接受 Tag 和 Symbol 输入；严格预处理的 proc 签名并不接受它们，因此文档将其标为兼容行为，而非向类型系统虚构联合输入。集合等非法输入仍在严格预处理被拒绝，JS 边界仍抛错。

## 验证

Calcit definition `:tests` 断言成功返回 Symbol 并可保留原文本；JS runtime 回归验证成功值、历史 Tag/Symbol 输入和非法集合输入。运行 core test、严格类型负例、core Dynamic 清单检查、Rust 测试与 `yarn check-all`。当前目标只收敛 `turn-symbol`；`turn-string` 的有效输入集合与后续 `to-*` 命名仍由 #1456 单独审阅。

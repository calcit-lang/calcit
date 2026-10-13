# Struct 接收者的 `.:field` 简写降级（#1988）

## 背景

`value.:field` 在文档中是 `(:field value)` 的接收者优先简写。此前对已知 Struct 接收者，预处理放行，但 native 运行时的 `MethodKind::TagAccess` 只接受 Map，报 `expected a hashmap`，且未知字段没有 `W_UNKNOWN_STRUCT_FIELD` 诊断。

## 决策

- 预处理阶段：接收者类型可解析为 Struct 时，把 `.:field recv` 改写为 `(:field recv)`，复用现有字段校验与 `&struct:nth` 降级，native / JS / WASM 共用。
- 运行时：TagAccess 对动态到达的 Struct 按字段读取，缺失字段报错。
- Map 接收者不改写，保持原有 `get` 语义（缺失键返回 nil），避免收紧 `reel.:store` 这类既有用法。
- JS：TagAccess 生成 `obj.get(tag)`，`CalcitStructValue.get` 已可读取字段；缺失字段返回 `undefined` 而不是抛错，这是动态路径上与 native 的差异。

## 限制

匿名 Struct 接收者不被改写，仍走运行时 TagAccess。

## 验证

`test-struct.main/TagShorthandEvent` 的 `:tests`；Rust 测试 `type_fail_tag_access_shorthand_matches_tag_call_diagnostics` 断言 `W_UNKNOWN_STRUCT_FIELD` 与 `(:missing ev)` 一致。

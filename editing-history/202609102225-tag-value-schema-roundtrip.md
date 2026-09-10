# Tag value schema round-trip / Tag 值 schema 往返

## Context / 背景

`calcit edit schema ... --code "quote 'Tag"` accepted the direct primitive
schema and stored a `CalcitTypeAnnotation::Tag`. Snapshot serialization then
produced the canonical `:: 'Tag` form, but serialized-content validation
rejected that same form. The edit command therefore reported success while a
subsequent `calcit edit format` could not canonicalize the snapshot.

`calcit edit schema ... --code "quote 'Tag"` 会接受直接 primitive schema 并
保存为 `CalcitTypeAnnotation::Tag`。Snapshot 序列化随后生成规范形式
`:: 'Tag`，但 serialized-content validation 又拒绝该形式，导致编辑命令报告成功
后，`calcit edit format` 无法 canonicalize 同一 snapshot。

## Decision / 决策

Treat `Tag` like the other concrete primitive value schemas during standalone
schema validation. Unknown standalone names still become `Dynamic` and remain
rejected, so accepting canonical `Tag` does not weaken typo detection. Tests
cover direct editor input, serialized validation, the actual `edit format`
handler, and reload of the preserved type.

在 standalone schema validation 中把 `Tag` 与其他 concrete primitive value
schema 一致处理。未知名称仍会解析为 `Dynamic` 并继续被拒绝，因此接受规范
`Tag` 不会削弱拼写错误检测。测试覆盖 editor 直接输入、序列化校验、实际的
`edit format` handler，以及保留类型后的重新加载。

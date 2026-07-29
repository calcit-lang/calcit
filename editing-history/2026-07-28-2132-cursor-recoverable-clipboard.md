# Cursor clipboard 可恢复提交与精简状态

## 概要

- `.calcit-cursor.cirru` 升级到 schema v3，继续读取 v1/v2；active、history 与 stack 不再重复保存完整 subtree preview，展示时从当前 Snapshot 重建。
- `cursor cut` 和 `cursor paste` 在修改任何目标文件前先生成 Snapshot 与 sidecar staged 文件，避免序列化或临时文件失败造成半成品。
- cut 按 sidecar→Snapshot 顺序提交，确保 Snapshot 提交失败时完整表达式已经存在于结构化 clipboard。
- paste 按 Snapshot→sidecar 顺序提交；若第二步失败，错误明确说明源码已经修改并禁止盲目重试。
- `cursor show` 构造真实节点和 focus preview 时复用同一次 Snapshot 读取，减少重复解析。

## 验证

- 新增 cut 第二阶段失败时 clipboard 已持久化的故障注入测试。
- 新增 paste cursor 提交失败时 Snapshot 已提交且错误包含 partial-success 指引的测试。
- sidecar round-trip 断言 schema v3 且不包含 `:preview`。
- `cargo test --bin cr cursor`
- `cargo clippy --bin cr -- -D warnings`


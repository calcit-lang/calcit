# 补充开放集合泛型检查顺序说明

时间：2026-09-13 06:36 +0800

## 调整

- 根据 #1046 review 复核 generic argument 的排序，并补充英文源码注释说明既定语义。
- review 建议将 concrete 参数排在开放参数之前，但这会让 #995 明确接受的 `Dynamic`/具体值双向安全传递失败：先绑定具体类型会把后续开放值误判为需要 narrow。
- 保留开放参数优先，使有意声明的 `Dynamic` 可先建立开放泛型绑定；需要具体能力的 callback 仍由既有测试稳定拒绝。

## 验证

- `cargo fmt --all -- --check`
- `cargo test dynamic_generic_bindings_preserve_transport_but_reject_callback_narrowing`

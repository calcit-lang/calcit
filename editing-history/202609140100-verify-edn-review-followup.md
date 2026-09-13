# 补充 #1081 review 修正

- 在 JSON integer 转换为 Cirru EDN 前显式拒绝会触发 Rust 饱和反向转换的 `i64::MAX` 与 `u64::MAX` 边界。
- verification CLI 的成功与配置失败 fixture 现在比较完整 JSON/EDN 语义 envelope，只规范化下划线与连字符形式的 key。

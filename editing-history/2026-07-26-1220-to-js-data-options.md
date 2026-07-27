# 2026-07-26 12:20 to-js-data options validation

## 修改概要

- 将 `to-js-data` 的第二参数改为 options map，并保留旧 boolean `addColon` 参数的兼容警告。
- 支持 `{} (:js-array true)`、`:js-object`、`:js-string`、`:js-number` 作为转换结果验证标记。
- 为 JS 集成测试和 `cr query examples calcit.core/to-js-data` 增加 options 示例。

## 验证

- `yarn compile`
- `cargo test -q --bin cr`
- `yarn try-js`
- `cr calcit/test.cirru query examples calcit.core/to-js-data`

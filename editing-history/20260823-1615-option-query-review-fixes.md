# Option 查询终点 review 修正

- RFC 的生态统计改为记录样本同步日期与默认分支状态，不包含本机绝对路径。
- `RuntimeMapMeta`、`RuntimeMapResponse` 的 schema kind 与 `defstruct` 对齐为 `Struct`。
- Option 迁移诊断只在能识别直接查询来源时推荐对应的 `get-or` 等 helper；普通 Option 值继续给出分支或方法建议。
- 增加诊断单元测试，覆盖直接查询与无可证明查询来源两条路径。

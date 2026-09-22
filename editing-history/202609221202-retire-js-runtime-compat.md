# 退役两处 JS runtime 兼容入口

- `to-js-data` 的第二参数只接受当前 options map；旧布尔写法现在给出明确迁移错误。需要保留 tag 键的冒号时使用 `:add-colon true`。
- 移除未被当前 codegen 引用的 `_$n_enum_def_$o_has_variant` 导出别名，保留 `&enum-def:has-variant?` 对应的正式导出。
- 运行时边界测试覆盖当前写法与旧写法的行为；Calcit 定义的语义未变，因此这次不增加重复的 Rust 或 Calcit 语义测试。
- 在继续退役 `%{}?`、旧 trait tag key 或 Struct metadata 之前，应先处理其真实生态调用点；它们不属于本次小范围 JS 兼容清理。

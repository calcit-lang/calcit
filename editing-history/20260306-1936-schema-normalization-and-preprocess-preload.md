# 本次修改记录

- 统一 schema 风格：清理 `:args` / `:rest` 中参数名，保留类型表达式，降低重复声明。
- 预处理前置 schema：在 `preprocess_defn` 中优先注入 schema hint，再进行类型与返回值检查。
- 程序装载增强：`program` 层为定义缓存并暴露 schema 查询能力。
- 快照规范化：`snapshot` 序列化时统一去除 schema 的 `:name` 字段与命名参数注解。
- 文档同步：更新 Agent 文档中 `hint-fn` / schema 写法示例到当前风格。

## 验证

- `cargo test` 通过。
- `yarn check-all` 通过。

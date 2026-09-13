# 收敛 enum 匹配与 Struct 字段访问

## 背景

Calcit 0.14.15 已发布 `tag-match-to-match-v1` 与 `required-struct-field-v1` 两条自动迁移规则。0.15 不再需要继续维护旧宏、兼容 warning、fix planner 及其路径和词法分析分支。

## 变更

- 使用已发布的 0.14.15 工具迁移主仓库 Calcit source 与 `:tests`，enum 模式匹配统一为原生 `match`。
- 删除不在主 suite 中、迁移后只产生静态 arity warning 的旧 `test-tag-match-validation` 独立夹具；有效行为继续由主 Calcit `:tests` 覆盖。
- 删除公开 `calcit.core/tag-match`、无 compiler consumer 的内部 `&tag-match-internal`，并同步 core schema inventory 与质量基线。
- 静态推断为 Struct 的 `(get value :field)` 改为稳定错误 `E_STRUCT_FIELD_OPTIONAL_LOOKUP`；字段访问使用 `(:field value)`。只有显式 `Dynamic` receiver 保留可缺失的运行时查询。
- 0.15 的 `calcit fix` 删除两条已完成使命的 planner 及辅助分析逻辑；若请求旧 rule ID，错误会指向 Calcit 0.14.15 的升级桥。
- 旧 `tag-match` source 直接报告 `E_REMOVED_TAG_MATCH`，同时给出原生 `match` 与 0.14.15 自动迁移命令。
- 更新 agent contract、语言文档和接口 smoke，避免继续推荐已删除的表层语法。

## 验证重点

- Calcit `:tests` 覆盖迁移后的 enum 分支、匿名 enum、错误 variant 与 payload arity。
- Rust 只保留 compiler 边界断言：稳定错误代码、迁移命令、静态 Struct 拒绝与显式 Dynamic 保留。
- `calcit fix` 测试确认当前规则仍可工作、退役规则给出版本提示，agent interface smoke 同步验证该契约。

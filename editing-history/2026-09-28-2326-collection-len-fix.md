# 2026-09-28 23:26 — 内建集合长度迁移边界

## 决策

#1455 的长度批次首选 `.len`，但不直接删除 Countable 或用文本替换所有 `.count`。List、Map、Set、String 的现有两种方法可在具体接收者上证明指向同一 core 计数实现，参数和返回类型一致；Struct 字段数、Enum payload 数及用户自定义 Countable 不纳入自动改写。

在现有 `calcit fix` 入口新增显式规则 `core-collection-len-v1`。只扫描 definition `:code` 中零参数方法调用，保留求值次数与顺序。类型未知、方法来源不一致或未知 macro 路径给出 review 提示，不选择业务语义。保留 revision、fingerprint 和 staged validation 的既有事务防护；不把规则放进已发布 preset。

## 验证

CLI 回归在临时 Snapshot 中附加 Calcit `:tests`，覆盖 List/Map/Set/String 的 `.count` 到 `.len` 迁移前后语义、四个建议的预览和应用、错误 revision 拒绝及幂等。另确认 Struct/Enum 与 quoted data 不改写，未知 macro 只需人工审阅。完整仓库门禁与真实消费者结果记录在关联 PR。

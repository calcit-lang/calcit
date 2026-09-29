# List 追加与集合长度的查询角色

## 背景

首批方法角色只复用了通用 method alias 规则。List `.add/.append` 和 List/Map/Set/String `.count/.len` 已有独立的显式 fix 规划器，却仍在查询中并列显示为 `proven`，Agent 难以选择首选名称。

## 调整

将这两组既有 fix 的“具体接收者、同一 core 实现、相同参数/变长参数和返回类型”证明抽成共用函数；查询角色和 fix 规划器调用同一个证明。List 的 `.add` 只有在 `.append` 同样得到闭合契约时才标为兼容；Map `.add` 指向另一个 entry 实现，绝不借用 List 规则。`.count/.len` 仅覆盖已证明的 List/Map/Set/String，各自验证实际 core 实现来源。开放类型不因为方法拼写相同而被推荐。

这次没有新增 fix 规则、preset 或顶层命令，也不改变运行时方法及旧调用的可用性。

## 验证

- 查询测试逐一验证 List `.add/.append`、四类接收者 `.count/.len` 的角色和 fix 名，并保持 Map `.add/.assoc/.dissoc` 无虚假角色、开放 List 不被错误标记。
- 既有 fix 回归覆盖 preview、apply、revision、幂等及不匹配来源；全量 Agent CLI、Rust 与文档检查继续运行。

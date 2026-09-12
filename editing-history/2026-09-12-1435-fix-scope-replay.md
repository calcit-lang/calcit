# Source fix apply 重放 preview scope

## 问题

CodeRabbit 指出 scoped preview 之后若只用 revision 执行无 scope 的 apply，命令会重新规划整个项目。revision 只能证明 Snapshot 内容未变化，不能证明扩大后的 suggestions 已经过同一轮审阅。

## 调整

- Agent 指南要求 apply 原样重复 preview 的 `--ns`、`--def`、`--rule` selectors。
- 用户文档的示例使用同一 namespace/definition scope，并解释省略 selectors 会重新规划更大范围。

这项约束不增加编译器分析规则，只让 preview 与 apply 的操作协议保持一致。

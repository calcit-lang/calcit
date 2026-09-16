# 将 external 字段判定绑定到声明 trait

## 问题

集中字段判定后，旧实现分别判断“trait set 中存在 external-object trait”和“任意 trait 声明该字段”。混合 trait set 中，这两个条件可能来自不同 trait，导致普通 trait 的字段被错误 lowering 为 JavaScript 外部属性访问。

## 修复

- 先解析实际声明字段的 trait，再只对该 trait 检查 external-object 元数据。
- 增加混合 trait set 回归测试，确认普通 trait 字段不会被误判，同时 external-object trait 自身字段仍能识别。

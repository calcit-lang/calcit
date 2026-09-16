# 集中 external-object trait 字段判定

## 背景

external-object trait 的前缀字段访问、后缀 tag 访问和预处理后的普通字段访问，都需要判断接收者类型是否包含 external-object trait，并确认字段已在 trait 中声明。此前三条路径分别展开了相同条件，后续调整判定规则时容易遗漏其中一处。

## 调整

- 新增内部辅助函数 `is_external_trait_field`，集中组合 trait 类型解析、external-object 元数据判定与字段声明查找。
- 三种字段访问 lowering 只保留各自的语法节点构造，复用同一判定入口。
- 不改变字段选择顺序、诊断或 JavaScript 名称映射语义。

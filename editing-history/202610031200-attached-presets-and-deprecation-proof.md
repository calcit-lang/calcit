# 附带 preset 与弃用调用证据

## 决策

`--include-attached` 复用既有规则覆盖 definition 的 `:tests` 与 `:examples`，包括冻结的 core 与 surface preset；不新增升级命令或兼容名注册表。每条规则基于前一步的 source 重新证明和定位，整个 metadata 区域仍合并为一次 guarded replacement。歧义或未知 macro 阻止整个区域写入，断言、tags 与业务默认值不变。

旧 data API 使用编译器已有诊断及迁移映射；`tuple?` 的值/定义含义不由工具代选。弃用函数使用原有 `:deprecated` 标签和定义文档，报告与 query context 共用这一来源。只补齐已提供首选入口的兼容名，不删除入口或修改运行行为；新增质量预算债务仅随非 patch 版本交付。

弃用报告保留 reader-resolved builtin 调用，并在普通函数解析成功时用现有 compiler source usage trace 判断目标与局部遮蔽。推荐方法允许复用旧内部 helper，不因 lowering 的实现名字而成为弃用调用。无法完成预处理的定义仍保留既有保守 source 报告，不把它宣称为完整类型证明。

## 验证

CLI 协议回归在临时 Snapshot 中通过结构化命令构造 Calcit 附带断言，验证 preview 不写入、revision 保护、原子应用、原断言重放与幂等。额外覆盖歧义、未知 macro、quoted data、局部同名参数、namespace alias 与推荐方法。

core 标签及文档仅通过 `calcit edit tags/doc` 修改，保留其他标签、schema、code 与测试。报告验收核对兼容函数的首选名和 query context 的同源提示。完整 CI、跨项目验证与 PR 门禁仍须另行完成，不以本地单项通过代替阶段交付。

# 外部 trait 的诊断 IR 序列化

## 问题与决策

Tabletwo 服务端能够通过严格预处理，但诊断 IR 遇到 `js-cast` 内已解析的
`Calcit::Trait` 时进入 `unreachable!`。最小复现保留了跨命名空间类型、字段、方法、
父 trait 与 FFI 名称映射，普通 native 执行和 JS 生成成功，只有 IR 失败。

为 trait 值补齐诊断表示，保存声明来源、成员名称、成员种类、原始类型、默认方法、
父 trait 与源声明的 FFI 元数据。默认方法复用普通函数的序列化，避免维护两份格式。
不把 trait 简化为短名称或空占位，也不改变预处理中的类型关系。

## 边界

- `program-ir.cirru` 仍是调试数据，不是应用部署包或可重新执行的 Snapshot。
- 不修改 `js-cast` 的检查及运行时行为，不增加 Dynamic、unsafe 或新公开入口。
- `&extract-code-into-edn` 原有运行时值限制保持不变。
- 没有修改 Tabletwo 源码、依赖、版本 pin 或测试预期。

## 验证

在现有 CLI 集成测试中，通过 dry-run 与 scoped revision 创建最小临时 Snapshot；
核对严格检查、native 执行、JS 生成及 IR 的结构化成员/继承/FFI 数据。此测试验证
序列化和生成文件边界，不能用只断言语言返回值的 definition `:tests` 替代。

真实 Tabletwo 使用未修改的 Snapshot 副本与原依赖：正式 alpha.3 重现原 panic，
候选修复成功生成 IR，且 `respo.css/StyleElement` 两个 String 字段及 FFI 元数据仍在。
原有依赖的 require-marker 告警继续保留；这不是应用完整迁移或正式版本配对验收。

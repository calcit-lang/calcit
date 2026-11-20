# deftype 类型槽保留声明合同

## 原因

#2883 中，入口类型槽绑定到没有额外 schema 的 deftype 时，resolve_type_slot 将该定义误作普通代表值进行静态值推断；deftype 的运行值是 Symbol，不能代表其声明的 AnyEnum 合同。

## 决策和边界

在代表值推断前识别显式 deftype，保留其符号引用，让已有别名关系展开声明。普通无 schema 代表值仍走原静态推断。补审发现 AnyStruct 的开放家族节点与旧 Custom 表示共用分支，原保护会误把该家族恢复成名义 TypeRef；现在保留显式开放家族的展开结果，名义 Enum/Struct 的构造器 schema 仍保留实例身份检查，递归防护及 Dynamic 门禁不变。值式别名的迁移提示保留 AnyEnum/AnyFn/AnyStruct 拼写，不将它们打印为旧 keyword 类型形式。

## 验证

在现有 CLI 集成测试中通过官方 edit 建立代表值与别名迁移，验证 AnyEnum 槽接受具名及匿名事件、拒绝 Number，并检查迁移提示；补测 AnyStruct 接受具名 Struct、拒绝 Number，并验证两个字段相同的不同名义 Struct 仍不兼容。该测试覆盖编译拒绝与 CLI 诊断，使用 Rust 集成测试；不新增统计器、迁移规则或类型放宽。

# 保留缺省 schema 与显式 Dynamic 的差别

在 #1307 的 helper 推断实验中发现，内存与二进制 Snapshot 已区分缺省 schema 和显式 Dynamic，但 Cirru EDN 保存与 `edit schema --clear` 都会写入显式 Dynamic。因此，即使函数体可推断，随后一次格式化或其他结构化编辑也会改变声明意图。

先修复这个前置问题：普通 CodeEntry 保存时保留缺省；clear 删除字段；显式 Dynamic 原样保留。现有 data definition 的规范化 marker 继续生效。definition revision 加入缺省标记，避免两种不同声明意图获得相同的局部编辑前置条件。没有修改严格类型门禁，也没有交付实验中的 helper 推断。

本次验证属于 serializer、CLI mutation 与 revision 协议，不是 Calcit 运行时语义，因此使用 Rust round-trip 与真实 CLI 集成测试，不增加重复的语言运行时测试。覆盖创建、显式声明、清除、无关编辑、格式化、二进制与 EDN round-trip、revision 区分和严格模式仍拒绝缺失契约。

后续 #1307 应把函数体推断与 query type-at/context 的证据对齐一起交付，再在 Respo helper 中验证删除冗余声明的收益；不允许仅放松编译器门禁而保留误导性的查询结果。

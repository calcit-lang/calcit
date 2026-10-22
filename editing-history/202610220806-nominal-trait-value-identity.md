# 对齐 Trait 值的名义相等、哈希与排序

关联 #2274，作为 #2195 定义元数据比较的前置修复，不代替后者的字段类型、泛型、约束和定义复制路径验收。

原 JS `=` 将同名独立 Trait 判为相等，host hash 却按对象身份分配，比较器未分类 Trait；同一组 key 的 SliceMap 与 trie Map 查询结果不同。native 已使用本次求值的 runtime identity，用户文档也已声明独立求值的 trait 不按名称合并。Impl origin 的短名比较同时固定了错误关系。

JS 使用一个 realm 私有的弱引用身份表，由真实 Trait 构造登记。别名保持身份，独立求值保持不同身份；hash 和 ordering 只读取这份身份，不修改或序列化定义对象。多个 runtime 副本共享同一表，避免模块局部计数碰撞；`instanceof` 以已登记对象识别同 realm 的真实 Trait，而不是新增可伪造的对象属性或依赖 prototype。构造后冻结的对象仍可比较和哈希。

Impl origin 使用同一个对象身份关系，定义类与 Impl 的排序位置对齐 native。内部身份 helper 不从 procs 入口重新导出，不新增 Calcit API、类型检查入口、诊断、统计门禁、proof token、Dynamic 或 unsafe。

使用既有 trait Snapshot 的新 namespace 和 definition `:tests` 验证别名/独立求值、先反向比较、Map/Set key、Impl origin、Enum impl 装饰和类型排序。原 source 与断言通过官方 transaction/revision 修改。保留旧宿主脚本的 duplicate dispatch、参数一次、候选不执行、双图与所有其他断言；把“独立同名 origin 相等”的旧偶然行为改为名义差异负例，同时保留真实同 origin 的相等正例。

宿主测试另外覆盖冻结对象、两个 runtime 副本及 SliceMap/trie Map/Set 表示，不新增运行器。原 runtime 从同一 Git 基线隔离构建，与候选运行同一个 Snapshot、同一 CLI 和原运行器进行 RED/GREEN 对照；不修改消费者断言，不用 compatibility 或后端排除获取通过。

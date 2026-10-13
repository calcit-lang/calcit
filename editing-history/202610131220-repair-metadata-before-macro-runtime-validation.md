# 显式元数据修正与运行加载分离

Quatrefoil Guidebook 的 package 声明与 app.* 源码不一致，同时有旧 Macro/Fn 契约。
旧 CLI 无法修 package，当前 CLI 又在修 package/schema 前先拒绝旧宏，形成迁移闭环。

增加一个 canonical source 元数据读取路径，复用现有解析、entries 校验与宏契约诊断。
它只供显式 package/schema 修正和事务的原始源码保留使用，不执行源码，也不猜契约。
schema 写入继续校验选中的宏，namespace 权限保持不变；事务结束仍用严格 loader
检查整个 Snapshot。普通查询、格式化、预处理和运行不切换到该读取路径。

package 改动保留原 EDN 字段，schema 改动只替换选中的字段。原子事务保留未修改
契约形式，并使用原始源码视图生成 revision 守卫。没有新增 fix 规则、错误码、
proof 特性、检查脚本或运行兼容模式。

回归放在现有 tests/edit_cli.rs，因为 namespace 权限、原子文件保留、事务 preview
和 loader 边界属于 CLI/host 行为，不能用语言 definition-attached tests 完整表达。
全量验证、真实下游临时副本、review 和正式发布的证据保留在关联 PR；本记录不
把未完成的发布或业务源码迁移写成已完成。

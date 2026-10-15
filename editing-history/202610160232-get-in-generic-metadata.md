# 泛型 Map 元数据路径读取的 lowering 边界

## 依据

Timegrass 在 #2080 的迁移中保留 `Map<StateKey,Dynamic>`，经维护者批准用现有 `get-in` 读取固定 `:cursor` / `:data` 元数据，并在消费处检查内容。#2106 的最小用例实际查询正常，但 strict proof 在生成的 `gen%` 中错误要求 Tag 等于任意调用者泛型 K。

原因是 typed literal path 优化只检查容器与 payload，却生成了具有更强 key 合同的 `get`。优化不能把原路径查询变成要求调用者额外证明的直接访问。

## 决定与兼容边界

每层 Map 在生成 typed `get` 前，复用现有调用证明，且不引入可绑定的 callee 泛型；仅在路径 key 独立满足接收者 key 类型时优化。未知或不匹配关系保留原公开 walker，空 Map 的 Never key 保持原例外。已证明路径继续保留精确 Option payload。

不改变公开 API、Map 写入、直接 `get`、List/String 索引和 Struct 边界，不新增动态合同、unsafe、迁移规则或静态证明入口。与 #1553 的改写后校验保持一致：修复产生无效调用的改写，而不是跳过校验。

## 验证

语言行为放在 `read-state-metadata` 的附带测试，覆盖 false / 0 / 空字符串、混合 cursor、无关 key、nil 路径语义、具体 payload 推断和单次求值，并通过现有 native / JS 共享运行器回放。Rust 只验证内部优化资格及现有 CLI 的 scoped proof、完整 strict workflow、嵌套报告、只读行为与直接 `get` 负例。

真实消费者仍须用正式发布的匹配 CLI/runtime 和对应模块重新验收；源码候选及旧 Respo 依赖不能代替这项门槛，0.30.1 尚未完成发布。

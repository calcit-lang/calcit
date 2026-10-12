# 保留 pair 映射的独立结果类型

## 决策

Timegrass 的泛型 StateKey 迁移暴露了一个与状态键无关的问题：输入为
`Map<String,Map<Tag,Dynamic>>` 时，明确构造 `[] key value` 的回调结果仍被视为
`List<Dynamic>`，不能传入 `List<List<Dynamic>>`。在 main 基线上，新增的 core
附带测试先稳定报 `E_CALL_ARGUMENT_UNPROVEN`。

`&map:to-list` 已保留外层 List；证据丢失位于 `&list:map-pair` 的公开可查 schema：
原合同只有开放 Fn 和 `List<Dynamic>`。用现有泛型 R 关联回调输出和 `List<R>`，
内部单元素 mapper 的返回 hint 同步为 R。运行时代码和 Rust 检查器不变，不新增按名字的特例。

## 边界

异构 pair 仍没有独立 K/V 类型证明，因此回调两个输入保持 Dynamic；不能从输出 R 反推输入。
原来将嵌套 Dynamic 换成 String 后通过的对照，不等于原合同具有精确结果类型：
显式开放参数会触发更严格的调用证明要求。

普通检查与 `fix --workflow strict --verify` 的返回证明策略不同。本次反例在实际调用点验证
开放结果不能进入 `List<Number>`，不把只声明返回类型当作完成所有独立返回证明。
完整 Timegrass 检查越过原列表链后，还报告容器传给 `comp-overview` 的 today 缺少 String 证明；
不据此修改业务合同或宣称客户端全部通过。

## 验证

- core `:tests` 覆盖嵌套开放 payload 的 List 结果、Number 结果、空输入、顺序/次数、输入不变及异常。
- 现有 `check-known-assertion.mjs` 覆盖标量冒充 List、开放 payload 传入具体消费者、未证明的回调输入；native/JS 均拒绝且不生成业务产物。
- 统一运行器实测 native/JS；WASM/WASI 仍显式不支持错误格式化依赖 `to-lispy-string`，效果测试还依赖 ref，逐项保留排除原因。
- 保留历史 API 合同门禁，并用现有工具生成 Dynamic 边界清单；清单记录开放 pair 输入，不以数量作为正确性门槛。
- 完整检查与外部项目结果记录在 PR；本地候选未发布，不替换用户全局正式 CLI。

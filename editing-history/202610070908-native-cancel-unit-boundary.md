# 原生任务取消的受检 Unit 边界

关联 #1797、#1529。Diary 使用已发布 alpha.15 时，显式取消 adapter 的
`&ffi-task-cancel` 返回值没有独立类型证明；默认取消虽然通过返回检查，却丢弃
宿主结果并补 Unit。native 实现的成功和重复 Closing 分支实际均返回 Unit。

两种 adapter 都在同一原生调用外使用既有 `decode-map-as ... 'Unit`，不修改
native ABI、reason 编码、Closing/终止/资源回收顺序或公共方法。真实返回值
只检查一次；原生异常在 decoder 之前传播，不扩大 Dynamic 或借用返回声明。

definition `:tests` 覆盖 Unit 保留、错误结果拒绝、正常方法上的 capability
错误以及 reason 的单次求值和异常。已有 CLI proof 回归同时检查两个 adapter
的 callable/return proof 和只读性；低层 ABI/lifecycle 与真实 std/WSS smoke
沿用原运行器。JS/WASM 不提供 native task capability，仍保持原 unsupported
边界。发布包和真实消费者须在 PR/main 门禁之后另行验证，不能把局部修复
当作 Diary 或整个 milestone 完成。

真实回放另发现不透明 capability 作为 reason 的既有序列化 panic，已在发布
alpha.15 和候选分别复现并单独记录为 #1798。本次不改 serializer，也不把
这一未修复条件描述成可捕获异常；后续发布前另行处理。

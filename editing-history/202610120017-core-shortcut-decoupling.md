# 集合谓词与旧公共 shortcut 解耦

`any?` / `every?` 原先引用公共 `foldl-shortcut`。该公共名字计划在消费者迁移后退场，但它们还需要 List、Set、Map 的原迭代及短路行为。当前 `fold-while` 的声明和实现只支持 List；直接替换会丢失 Set/Map，先转换集合则会增加全量分配，也不能未经验证改变遍历顺序。

本次只将两个 core 定义接到已经存在的内部 `&list:foldl-shortcut`，不增加 primitive、重命名内部注册、复制后端实现或扩大类型。它与原公共 proc 复用同一 native/JS 执行路径；WASM 的已有谓词 lowering 不变。内部路径仍保留实现角色，不成为推荐给应用的 API。

用户语义写在原定义的 `:tests`：三个集合上的决定性结果只调用 predicate 一次；空集合不调用 predicate，`any?` 返回 false、`every?` 返回 true。测试用 Ref 观察真实 callback 次数，native/JS 回放相同 AST；WASM/WASI 按既有 Ref unsupported 边界明确排除这两个新增观测测试，原谓词测试继续回放。

这只是公共名字依赖解耦，不宣称底层 bool shortcut 协议已经删除。公共 `foldl-shortcut` 及迁移规则仍保留，消费者需要按 `fold-while` / `ControlFlow` 的实际合同人工迁移。最终删除与 0.30.0 发布仍须逐项满足消费者、发布窗口、语义与依赖门禁。

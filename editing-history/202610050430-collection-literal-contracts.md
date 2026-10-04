# 集合字面量复用期望合同检查

Respo 的 `Element.event` 保存 `Map<Tag,JsNullish<EventHandler>>`。完整签名回调与 nil 均能独立证明，但同质集合合并后产生 Dynamic，使原先的跳过 nil handler 测试失败。不能以删除断言或放宽字段替代修复。

沿已有返回表达式检查方向，在有闭合期望合同时逐项验证预处理后的 List/Map/Set。字段、返回与显式字面量 `assert-type` 复用这一条规则；底向上的推导、运行值和开放参数迁移策略不变。局部别名证据记录被证明的合同，子成员不能复用不同合同，shadowing 清除旧证据。检查继续有节点和深度上限，不增加全局证据缓存或统计机制。

JsNullish 可以包装已证明的集合；Optional 不因此转换为 JsNullish。Fn 输入/输出、可变 Ref 和 raw primitive 的既有规则保持。异步 Promise 采用只作用于整个返回值，字面量成员不会隐式 await。

用户语义写入 definition `:tests`，由现有运行器在 native 与生成 JS 重放；负例拒绝错误成员、签名与开放值，并确认无应用产物或源文件修改。真实消费者保持 Respo 源码和测试不变，验证 native 原测试与默认 browser-entry 检查/codegen。完整 UI 和发布版依赖验收独立进行。

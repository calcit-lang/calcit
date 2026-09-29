# WASM 使用类型证据选择 trait 实现

关联 [#1515](https://github.com/calcit-lang/calcit/issues/1515)。`to-string` 是普通的 `T: ToString` 泛型函数，不为它增加专用 WASM 分支。预处理阶段已经把受 trait bound 约束的方法调用转成保留 trait 来源的 `&trait-call`；WASM 现在复用同一份 trait/impl 解析逻辑，凭具体接收者类型选择静态目标，并在泛型调用点特化函数体。

这次仅接受参数个数固定、实现目标可静态调用的路径。调用点特化会沿静态 helper 调用图传递，因此多层泛型 helper 也能取得具体类型。运行时参数仍按顺序、各求值一次。类型未证明的泛型 export、Dynamic、开放的一等函数或 spread 调用会明确拒绝；重复名义实现不会按顺序猜测。小数的 WASM 文本格式化仍由 #1516 负责，不能把本次 trait lowering 误认为已支持小数结果。

验证包含定义附着的 Calcit 测试、native/生成 JS/真实 WASM export 的对照、用户自定义名义 trait，以及 Wasmtime 49.0.1 上 WASI 0.3 component 的实际执行。未在本次引入新的语言语法或 CLI 入口。

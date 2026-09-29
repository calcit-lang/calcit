# FFI 生命周期方法显式标记效果

`FfiTask` 的取消和 `FfiResponse` 的完成都会改变宿主 capability 状态，属于 #1457 选定的资源生命周期效果。首选方法改为 `.cancel!`、`.cancel-with!`、`.resolve!`、`.reject!`。旧方法暂留兼容；新旧方法在 trait implementation 中指向同一个内部 helper，不新增宿主入口，也不改变泛型、`Unit` 返回、失败或 exactly-once 规则。

Calcit definition `:tests` 构造带静态返回类型的延迟调用，验证四个新方法能够在 nominal wrapper 上通过预处理和类型检查，但不会用虚构的 raw capability 实际执行。取消、响应竞争、重复完成、过期和释放必须继续由现有 native host/FFI 边界测试验证。JS 与 WASM 没有 native dylib capability，本次不扩大 backend 支持。

文档与 `query examples` 优先展示新名。升级旧调用仍需先证明接收者为 core `FfiTask` / `FfiResponse`，不能根据 `.cancel` 等同名字符串批量改写用户方法；跨模块消费者和受控 fix 留在 #1457 后续阶段。

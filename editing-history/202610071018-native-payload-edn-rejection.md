# 原生文本 payload 的受检编码

真实 std 取消回放中，不透明 task capability 在已发布工具链也会触发 Cirru EDN formatter 的 AnyRef panic。内存 EDN 可以保留 AnyRef；原生文本传输必须在格式化前拒绝它，不能使用展示用 sanitizer 改写真实 payload。

在既有 ffi_abi 编码边界复用一个递归检查，覆盖 List、Set、Map 的 key/value、Struct、Enum 和 Atom。只有拒绝时构造数据路径；合法值使用原 formatter 保持字节，Quote 只包含 Cirru 语法节点。同步/async/blocking 请求与取消、response resolve/reject、blocking callback 返回值复用边界，不增加 Calcit 入口、动态类型放宽、信任表或新的诊断。

取消和 response 先编码再访问生命周期状态，保留错误时不执行 cancel hook、不移动为 Closing、不消费 response 的顺序。异步 Unit 与 blocking 忽略结果的 nil 沿用既有协议。

Rust 覆盖 serializer、不可由普通语言表达式构造的 opaque capability、提前拒绝及 C callback 错误 buffer 的释放。真实 std 的 Calcit fixture 使用普通 `.cancel-with!` / `.cancel!` 与 `try`，验证顶层和嵌套拒绝、reason 单次求值及错误后正常取消；配合 trace/metrics 验证单次 hook、terminal 与资源归零。保留完整 Cargo、check-all、文档和原 std/WSS 原生门禁，再等待最新 HEAD review/CI、精确 main 和实际发布包回放。

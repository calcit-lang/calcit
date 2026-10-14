# 日志的 Unit 返回合同

维护者确认 0.30.0 将 println/eprintln/echo 的真实返回统一为 Unit。旧 native 成功与
静默路径返回 Nil；仅补类型会制造假证据。此次同时调整实际 native 返回、JS 明确
丢弃宿主返回，以及现有注册元数据中的可选 ProcTypeSignature。只有明确签名提供
callable 证据，默认 None；log/io 标签、名字和任意 registered function 不意味着 Unit。

注册函数与 local callable 复用既有泛型返回关系，没有新诊断、检查入口、规则或预算。
JS 的 Unit 仍是 undefined；WASM/WASI 仍沿已有 Unit 编码，不增加第二套运行时类型。

定义测试保留日志内容、Unit callback、零参数、stderr、求值顺序与错误传播。原 host
测试验证静默后的真实结果、stdout/stderr、宿主非 Unit 返回被丢弃、宿主异常原样传播；
WASI 原 stdin/stdio 验收验证真实 Component 通道和实参失败前不写部分日志。
WASI lowering 原来边求值边输出，改为先保存全部实参再输出，复用一个私有 helper。

回放发现原 marker parser 丢空行，core WASM 零参数日志没有事件；修复 parser 和对应
lowering后既有正例四后端一致。core WASM 单值 host import 无 stderr 区分、多参数
逐事件输出是另一个 ABI/语义差异，独立 issue 跟踪，不当作 Unit 类型已完成全平台 IO。
try 在当前 WASM/WASI 仍明确 unsupported；保留 native/JS 错误测试与 WASI 实际 trap
验收，不通过放宽断言、unsafe 或抹除失败达成发布。

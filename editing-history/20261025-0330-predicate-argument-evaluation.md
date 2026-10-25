# 类型谓词优化保留参数求值

正向类型谓词的 specialization 曾直接返回 `true`，丢弃参数。静态 `Bool` 或 `List<Number>` 只能证明值的类型，不能证明生产该值的调用没有日志、不会失败或必定终止。给函数补充精确合同不应消除这些行为。

复用已有内部 `&let () argument true` 顺序表示：已经预处理的参数保留一次，返回已知真值，不再次预处理，不写回用户 Snapshot。未知或不匹配类型仍走原谓词。没有新增公共入口、类型规则、诊断、Dynamic/unsafe 或纯度统计。

完整 CI 进一步发现，已经求值的局部引用不应再强制物化为运行时值：WASM 的静态特化闭包因此误报逃逸。局部值已在绑定或调用参数处求值，读取本身没有用户副作用，继续保留其常量优化；真正的调用和其他可能求值的表达式仍走内部顺序表示。原 `async-scope-keeps-local-callbacks-sync` 测试保留并复验，不通过排除该用例掩盖回归。

语义回归附在非 core 的 `test-types-inference.main/test-fn-inference`：一次性有序日志与正常返回共用一项，异常传播、开放值及 Proc alias 控制共用另一项。日志用例沿现有统一运行器在 native/JS/WASM/WASI 执行；原宿主脚本断言完整预期 trace，防止各后端一起漏求值时比较仍假通过。异常捕获和一等 Proc alias 目前不属于 WASM 子集，保留 native/JS 验收，不增加排除；WASM/WASI 的真实失败通过已有 nil-predicate 宿主 fixture 验证。

初次测试准备曾错误地把内建 `bool?` 放在局部绑定位置，所有后端均正确拒绝；之后的 Proc alias 控制在 WASM 也明确 unsupported。二者不属于本次优化回归，修正 fixture 并保留支持范围，不把这些失败改写为通过。原测试与 CI job 保留，不增加平行运行器。提交前及合并前须按 AGENTS.md 完成完整检查、最新提交实际审查与精确 main 验证。

维护者随后确认，如实标注被旧谓词漏求值遮住的两项既有能力边界：原生调试 formatter 在 JS/WASM/WASI 不支持，开放 Fn 返回闭包在 WASM/WASI 不支持。仅补齐这两个原测试的对应后端排除，保留其源码及支持后端验收；不排除局部闭包引用回归。既有 nil-predicate 宿主脚本通过同一运行器显式重放这些排除项，并检查真实的成功或原 unsupported 诊断，避免仅凭排除表或被删除的调用宣称成功。没有为旧调试 API 新增后端实现，也不宣称不透明函数值已可在 WASM 传输。

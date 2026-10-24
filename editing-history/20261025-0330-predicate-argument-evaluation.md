# 类型谓词优化保留参数求值

正向类型谓词的 specialization 曾直接返回 `true`，丢弃参数。静态 `Bool` 或 `List<Number>` 只能证明值的类型，不能证明生产该值的调用没有日志、不会失败或必定终止。给函数补充精确合同不应消除这些行为。

复用已有内部 `&let () argument true` 顺序表示：已经预处理的参数保留一次，返回已知真值，不再次预处理，不写回用户 Snapshot。未知或不匹配类型仍走原谓词。没有新增公共入口、类型规则、诊断、Dynamic/unsafe 或纯度统计。

语义回归附在非 core 的 `test-types-inference.main/test-fn-inference`：一次性有序日志与正常返回共用一项，异常传播、开放值及 Proc alias 控制共用另一项。日志用例沿现有统一运行器在 native/JS/WASM/WASI 执行；原宿主脚本断言完整预期 trace，防止各后端一起漏求值时比较仍假通过。异常捕获和一等 Proc alias 目前不属于 WASM 子集，保留 native/JS 验收，不增加排除；WASM/WASI 的真实失败通过已有 nil-predicate 宿主 fixture 验证。

初次测试准备曾错误地把内建 `bool?` 放在局部绑定位置，所有后端均正确拒绝；之后的 Proc alias 控制在 WASM 也明确 unsupported。二者不属于本次优化回归，修正 fixture 并保留支持范围，不把这些失败改写为通过。原测试与 CI job 保留，不增加平行运行器。提交前及合并前须按 AGENTS.md 完成完整检查、最新提交实际审查与精确 main 验证。

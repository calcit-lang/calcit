# 前缀方法的回调类型传递

关联 #1390、#1307。在闭合 helper 的多实例化验收中发现，前缀 `.map` 会先检查
回调体，未使用已知 receiver 的 `List<T>` 关系；回调中的普通 `.add` 因缺少
receiver 类型失败。后缀方法已有对应的预期参数解析。

修复只在前缀普通方法参数预处理时复用 `expected_method_argument_types`：先处理
receiver，再将其泛型绑定后的函数参数类型传入回调；离开参数时恢复原上下文，
包括错误路径。非方法调用保留原行为，出现 spread 后不猜测后续参数位置。
未增加分析服务、类型规则或 source rewrite，也不推断有未知输入的公共函数。

验收使用 `calcit/test-helper-inference.cirru` 中的 definition `:tests`：Number/String
容器共享 `.map`，回调使用普通 `.add` / `.count`，验证独立实例化与前后缀等价。
同一 AST 经现有脚本运行 native、JS、WASM；`helper-lengths` 的 query context 和
type-at 应共同报告 `Fn () -> List<Number>`，而源码仍没有 root schema。
CLI 负例继续拒绝不相容回调，也验证不能从单个具体调用猜出无声明函数的参数契约。

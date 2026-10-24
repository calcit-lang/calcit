# 保留函数映射的参数与结果类型

`&fn:map` 是 Fn 的 `.map` 方法入口，运行时为一元组合 `fn(x) = f(g(x))`：先调用 g，再把结果交给 f。参数合同早已声明 `f: Fn<B> -> C`、`g: Fn<A> -> B`，但返回只写了裸 `Fn`，新建函数的参数 x 因此没有 A 的证据。普通检查通过，独立 proof 与完整 strict 却在调用 g 处报 `E_CALL_ARGUMENT_UNPROVEN`。

只把返回 schema 改成 `Fn<A> -> C` 不够：声明本身不是证据，返回函数的参数仍是 Dynamic，并新增返回不成立的诊断。修复同时在返回函数体内用已有的 `hint-fn` 声明参数 A 与返回 C，让 checker 从函数体自身证明这一合同；不增加按函数名的推断规则、Dynamic 放宽或 unsafe。运行时代码、求值顺序与错误传播不变。

定义 `:tests` 覆盖直接调用与 `.map` 方法、跨类型组合、各调用一次且 g 先于 f、输入不变，以及 g 失败时 f 不被调用。native/JS 复用现有定义测试运行器；WASM/WASI 实测在闭包值、方法接收者特化与 `ref` 处编译拒绝，按现有格式登记为 unsupported 排除。

证明回归把新定义复制到临时非 core 项目，普通检查、`concrete-return-proof-v1`、`callable-contract-proof-v1` 与 `fix --workflow strict --verify` 均通过；旧合同、只改返回、用 B 冒充 A、用 String 冒充 C、直接返回未知签名 Fn 均仍被拒绝，检查不改写 Snapshot。`calcit/test-traits.cirru` 的完整 strict 中 `&fn:map` 的报错消失，同类缺口随后出现在 `&fn:bind`（返回同样是裸 Fn），留给单独处理。

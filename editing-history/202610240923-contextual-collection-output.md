# 集合回调的上下文输出槽与词法变量分离

关联 [#2121](https://github.com/calcit-lang/calcit/issues/2121)。`Fn<T,U>(List<T>, Fn<T>→U) → List<U>` 中，`map xs f` 能通过独立证明，等价的 `map xs $ fn (value) $ f value` 却把临时输出槽当成刚性的返回声明，报出临时变量与外层 `U` 不同。实际 core `&list:apply` 的嵌套 map 也受影响，不需要 concat 就能复现。

前缀 `.map` 还有同类信息丢失：入口已取得方法参数类型和 callee 的泛型归属，但只把参数类型传给回调预处理。将已有的归属信息一并转发，与后缀方法共用同一个上下文，不另建方法专用的证明规则。

## 约束

- 一个回调同样需要从实际函数体推导输出，不以回调数量决定临时槽的所有权。
- 已有词法身份的变量仍然刚性；不能因为名称与临时槽相同，就把外层变量交给 callee 重新推断。
- 复用既有上下文回调处理：放开尚未推断的临时输出约束，不等于提供实际输出证据。最终 callback 合同和外层返回仍使用实际函数体的结果证明；未知或显式 Dynamic 的结果不能借用具体返回声明。
- 不新增公开 API、诊断编号、固定改写、函数名例外或类型统计门禁，不修改 core 集合的原 schema。

## 验证要求

合法行为放在原类型推导 fixture 的 definition `:tests`：普通函数与 `.map` 方法都保留 Number/String 输入输出关系、空列表及输入不变；类型参数命名为 `MapOutput` 时也保留原词法身份。显式开放结果可以被集合保存和传递，但测试不假装已经知道元素的具体类型。

沿原 proof harness 从官方 query 复制真实 helper 和 `&list:apply` 的 code/schema，使用 dry-run 与 revision 守卫构造隔离项目，验证普通、独立返回与完整 strict，且检查不改写 Snapshot。五组合法 `:tests` 使用原运行器在 native、JS、WASM 回放；反例覆盖不相关输出变量、具体 String 冒充泛型和 Dynamic 冒充具体泛型。既有同名 binder、回调方差与跨后端回归保持原门槛。

# 持久集合更新术语：Set `.add` 的受控迁移

## 决策依据

本阶段保留 Map `.assoc/.dissoc` 与 Set `.include/.exclude`，它们都返回新集合，不引入可能暗示原地修改的 `.insert/.remove`。Set `.add` 与 `.include` 在 core impl 中都指向 `calcit.core/include`，因此可以在静态 Set receiver、同实现、同签名和稳定源码路径的条件下提供显式 fix。没有将它加入已发布 preset。

Map `.add` 是另一种操作形状：传入长度恰为 2 的 List entry，先断言形状，再调用 `&map:assoc`。现有 `List<P>` schema 未证明 K/V 分别对应 entry 的两项，失败与参数求值也不能靠方法名替换保持。因此本规则不处理 Map `.add`，也不更改用户自定义 `Add` trait 或同名方法。

## 验证边界

用户语义由 `calcit.core/include` 的 definition `:tests` 和 fix fixture 的 attached Calcit 测试覆盖；共享的命名契约脚本还从同一定义在 native、生成 JS、core WASM、WASI 0.3 Component / Wasmtime 执行。Rust CLI 测试只验证预览、来源证据、revision、防重复应用、应用后行为、Map/quoted/unknown-macro/自定义方法负例。真实消费者 Respo 当前使用 `assoc/dissoc`，未找到 Set `.add/.include/.exclude` 接收者调用；不能把其 40 项 definition 测试通过解释成 Set 源码迁移已验证。Map/Set 其余词汇验收继续由 #1479 追踪。

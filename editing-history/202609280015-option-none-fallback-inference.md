# 空 Option 的 fallback 类型推断

`Option<T>` 的空值不携带 `T`。以前 `%none` 或 `Option :none` 被表示为 `Option<Dynamic>` 时，`option:unwrap-or` 会先从接收者绑定 `T=Dynamic`，再忽略后面的具体 fallback 类型。这妨碍了严格类型检查，也让安全的 helper-to-method 改写仍需人工确认。

现在只有表达式被证明是 `calcit.core/%none` 或 core `Option :none`，且没有 payload 时，泛型绑定才跳过接收者的 `Dynamic`，让 fallback 决定 `T`。普通 `Option<Dynamic>`、`:some`、同名的非 core 构造器不获得这个特例。`fix core-option-method-v1` 只在源码引用追踪证明 `%none` 来自 core、fallback 类型具体、目标方法分派已证明时自动改写；否则仍要求人工确认。

Calcit `:tests` 覆盖两种空值写法，CLI 测试覆盖类型查询与改写的预览、应用、幂等，低层 Rust 测试锁定拒绝范围。WASM 样例改用 `(%none) .unwrap-or 7`，并由完整 WASM 脚本验证。此改动没有把任意动态 Option 转成静态 Option，也没有改变运行时值或扩大 JS FFI 边界。

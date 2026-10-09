# 已证明 Nil 的 Optional 单态化

0.29.0 转换契约验收补齐 `nil->option` 的附带测试后，正式 alpha.24 在 native/JS 通过，但 WASM/WASI 拒绝 `nil->option nil`。逐个回放原断言，false、0、空串和 String/Number 调用都可编译，嵌套 Option 四后端正常。

`resolve_static_specialization_types` 已检查实际参数与声明的类型关系；Nil 可以传给 Optional<T>，但不存在 payload，不能据此绑定 T。原本随后要求所有声明参数完全解出，错误拒绝了已有具体 Nil 证据的调用。

修复仍使用通用 specialization：仅在实际参数已证明为 Nil、声明参数为 Optional 时，不要求 phantom payload 的类型变量已绑定。传给 lowering 的参数类型保持 Nil；不捏造 T=Nil，不改变返回类型证据、schema、参数求值次序或 scalar ABI。其他参数继续完成原绑定检查；动态/开放参数和有真实零值歧义的 Optional<Number> 不因此获得支持。

用户语义保存在首选入口的两个 Calcit `:tests` 中，使用普通调用与 `.unwrap`，通过现有统一运行器重放同一 AST；不替换为 primitive/native call，不修改预期或添加排除。验证同时包括原 strict 负例、四后端执行、fmt/Clippy/Rust 与 latest-HEAD CI。正式 alpha 的复现和未发布修复 CLI 的结果分开记录在 PR 中，合并及精确 main 验证前不称完成。

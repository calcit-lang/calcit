# Option 短名的来源类型证明

## 问题

`infer-raise-left/right` 的 `value` 已由编译器推断为精确 `Option<Number>`，`query type-at` 也证明 `.unwrap` 指向 core 方法，但 `core-option-method-v1` 仅接受全限定 `calcit.core/Option`，使合法的短名 `Option` 无法自动迁移。

## 处理

改写规则复用预处理器现有的命名空间类型引用解析，先将推断类型中的短名按声明命名空间解析，再检查它是否确实指向 `calcit.core/Option`，并继续核对同一方法契约。审查还指出跨模块返回类型不能在调用者命名空间重新解释，因此可调用对象的声明 schema 和编译后回退契约先在被调用定义的命名空间解析，再绑定调用者传入的泛型实参。项目本地同名 `Option` 保持待审阅；不按字符串猜测，也不扩大 Dynamic。用该规则迁移两个已有 `if/raise` 的 Calcit 测试定义，保留其原有 `:tests`。

## 验证

CLI 测试针对两个真实 Snapshot 定义检查预览、应用、幂等及原有 Calcit `:tests`；另造本地同名和跨模块同名 `Option` 负例，验证声明方来源不会丢失，同时确认泛型传参仍保留调用者自己的类型。按仓库标准执行 Rust、Calcit、JS/WASM 与文档检查。

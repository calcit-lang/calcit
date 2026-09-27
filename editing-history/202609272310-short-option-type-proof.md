# Option 短名的来源类型证明

## 问题

`infer-raise-left/right` 的 `value` 已由编译器推断为精确 `Option<Number>`，`query type-at` 也证明 `.unwrap` 指向 core 方法，但 `core-option-method-v1` 仅接受全限定 `calcit.core/Option`，使合法的短名 `Option` 无法自动迁移。

## 处理

改写规则复用预处理器现有的命名空间类型引用解析，先将推断类型中的短名按声明命名空间解析，再检查它是否确实指向 `calcit.core/Option`，并继续核对同一方法契约。项目本地同名 `Option` 保持待审阅；不按字符串猜测，也不扩大 Dynamic。用该规则迁移两个已有 `if/raise` 的 Calcit 测试定义，保留其原有 `:tests`。

## 验证

CLI 测试针对两个真实 Snapshot 定义检查预览、应用、幂等及原有 Calcit `:tests`；另造本地同名 `Option` 负例，确保不能被当作 core 类型自动改写。按仓库标准执行 Rust、Calcit、JS/WASM 与文档检查。

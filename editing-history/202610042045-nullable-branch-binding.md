# 分支结果经过局部绑定时保留可空类型

## 需求与原因

Diary 从 `Map<String, Diary>` 查询日记，解构 `Option` 后将 `Diary` 或 `nil` 赋给局部变量，再构造带可空字段的 `ClientStore`。直接把 `match` 放在字段位置能够通过，经过 `let` 却被推断为 Dynamic。最小复现已归入 #1761，并关联 #1553 的改写后类型证据回归。

## 设计

复用 `if` 与 `match` 的分支合并函数：两个分支没有兼容的共同类型，且其中一个明确为 `Nil` 时，结果为另一分支的 `Optional<T>`。保留既有的 Never、Dynamic 和兼容类型合并优先级；不增加诊断、转换语法或业务专用规则。

`Optional<T>` 在这里是保留已有可空字段语义的推导结果，不新增公共 API 的旧式 Optional 签名。`Option<T>` 与 `nil` 合并后是 `Optional<Option<T>>`，不会把裸 `nil` 当作 `Option :none`。既有可空类型和两个 Nil 不重复包裹。

## 验证与边界

- 在现有 `def-value-schema.cirru` fixture 的 definition `:tests` 中覆盖直接/局部 match、两个 if 分支顺序、局部别名、已有可空字段、纯 nil 和 nominal Option。
- 扩展现有 `check-known-assertion.mjs`，把同一测试 AST 在 native 与 JS 回放；非空字段、不同 nominal 类型、错误 payload 和开放 if 分支仍须拒绝，JS 拒绝时不得生成应用代码。
- 已发布 alpha.2 在新增 7 个测试中失败 4 个；候选修复全部通过。Diary 原服务端入口严格检查和 JS 生成恢复，无需修改该业务表达式。
- 另发现已发布 alpha.2 的开放 Map payload 在 match 后可能延迟到 runtime 才拒绝错误字段，单独记录为 #1762；本次不能宣称所有 Dynamic 路径都已覆盖。
- 本地采用 no-WASM 构建；完整 WASM/WASI 门禁仍由 PR Actions 验证，不以 native/JS 结果替代。

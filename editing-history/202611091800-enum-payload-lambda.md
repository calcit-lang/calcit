# enum 载荷里的 lambda 使用声明的 Fn 类型（#2644）

## 决策

- `%:: Patch :mount (fn (t) …)` 与 `Patch :mount (fn (t) …)` 都降为 `&%::`。预处理这个构造的载荷时，若 variant 声明的载荷类型是不含 enum 自身泛型参数的具体类型，就把它作为 lambda 的期望类型，和 Struct 字段、函数实参的做法一致：参数 `t` 得到 `Task`，`(:titel t)` 报 `W_UNKNOWN_STRUCT_FIELD`。
- 载荷类型含 enum 泛型参数（如 `Option :some`）时不提供上下文，仍由载荷推断绑定参数。
- `redundant-hint-fn-v1` 随之覆盖这个位置：与声明载荷类型完全相同的 `hint-fn` 被报告为冗余。

## 验证

- `cargo test --test strict_check_cli -- enum_payload_lambdas`（fixture `tests/fixtures/enum-payload-lambda.cirru`）。
- `cargo test --test fix_cli -- redundant_hint`（新增 `h8-enum-payload`）。

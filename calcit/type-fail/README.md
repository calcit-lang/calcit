# type-fail fixtures (schema)

这个目录存放**预期在 preprocess 阶段失败**的示例，用来验证最近增加的 `:schema` 类型描述能正确生效。

## 运行方式

在项目根目录执行（都会返回非 0）:

- `cargo run --bin calcit -- calcit/type-fail/schema-required-arity.cirru --check-only`
- `cargo run --bin calcit -- calcit/type-fail/schema-rest-missing.cirru --check-only`
- `cargo run --bin calcit -- calcit/type-fail/schema-rest-unexpected.cirru --check-only`
- `cargo run --bin calcit -- calcit/type-fail/schema-kind-mismatch.cirru --check-only`
- `cargo run --bin calcit -- calcit/type-fail/schema-call-arg-type-mismatch.cirru --check-only`
- `cargo run --bin calcit -- calcit/type-fail/trait-method-generic-receiver-mismatch.cirru --check-only`
- `cargo run --bin calcit -- calcit/type-fail/generic-where-bound-mismatch.cirru --check-only`
- `cargo run --bin calcit -- calcit/type-fail/type-slot-record-call-arg-type-mismatch.cirru --check-only`
- `cargo run --bin calcit -- calcit/type-fail/type-slot-bind-unknown.cirru --check-only`
- `cargo run --bin calcit -- calcit/type-fail/type-slot-bind-duplicate.cirru --check-only`

其中：

- 前 4 个会触发 `schema mismatch while preprocessing definition`（定义时校验）。
- `schema-call-arg-type-mismatch.cirru` 会触发基于 schema 的函数参数类型告警，并在 `--check-only` 下被当作错误处理。
- `trait-method-generic-receiver-mismatch.cirru` 会验证泛型方法根据 receiver 的 `Option<String>` 绑定其 fallback 类型，并拒绝 `Number` fallback；同时验证 `.and-then` callback 不能返回裸 payload。
- `generic-where-bound-mismatch.cirru` 会触发 `W_GENERIC_WHERE_BOUND_MISMATCH`，验证泛型 `:where` 约束在调用点能被发现，并在 `--check-only` 下被当作错误处理。
- `type-slot-record-call-arg-type-mismatch.cirru` 会验证 `bind-type` 绑定 struct 实例后，`*slot` 参与调用点类型检查。
- `type-slot-bind-unknown.cirru` 会验证未声明 slot 的 `bind-type` 会直接失败。
- `type-slot-bind-duplicate.cirru` 会验证同一个 slot 重复绑定会直接失败。

## 自动化测试

这些 fixture 已接到 Rust 测试里，会随 `cargo test` 一起运行。

- schema mismatch fixtures：断言最终错误文本包含 `E_SCHEMA_DEF_MISMATCH`
- call-site arg mismatch fixtures：断言产生 `W_FN_ARG_TYPE_MISMATCH` / `W_GENERIC_WHERE_BOUND_MISMATCH`
- type-slot hard-fail fixtures：断言错误文本包含具体 slot 绑定失败原因

相关测试位于 [src/bin/calcit.rs](src/bin/calcit.rs)。

日常单独跑这组测试时，可以直接使用：

- `yarn test-fail`

这个命令会执行 `cargo test -q --bin calcit type_fail_`，专门覆盖这批 type-fail / schema-fail fixture 对应的测试。

## 当前相关 code

- `E_SCHEMA_DEF_MISMATCH`：定义与 `:schema` 的 `:kind` / `:args` / `:rest` 不匹配
- `W_FN_ARG_TYPE_MISMATCH`：用户函数调用参数类型不匹配
- `W_METHOD_ARG_TYPE_MISMATCH`：静态方法调用参数类型不匹配
- `W_DYNAMIC_NOMINAL_METHOD_RECEIVER`：Option/Result method 的接收者仍为 Dynamic，需先收窄或在明确边界使用函数形式
- `W_PROC_ARG_TYPE_MISMATCH`：内建 proc 参数类型不匹配
- `W_CORE_FN_ARG_TYPE_MISMATCH`：`calcit.core` 函数参数类型不匹配
- `W_FN_RETURN_TYPE_MISMATCH`：函数声明返回类型与函数体实际返回类型不匹配
- `W_GENERIC_WHERE_BOUND_MISMATCH`：泛型绑定后的实际类型不满足 `:where` trait 约束
- type-slot fixture 额外覆盖：struct 绑定、未知 slot 绑定、重复绑定、跨程序加载的 slot 状态清理

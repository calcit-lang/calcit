# type-fail fixtures (schema)

这个目录存放**预期在 preprocess 阶段失败**的示例，用来验证最近增加的 `:schema` 类型描述能正确生效。

## 运行方式

在项目根目录执行（都会返回非 0）:

- `cargo run --bin cr -- calcit/type-fail/schema-required-arity.cirru --check-only`
- `cargo run --bin cr -- calcit/type-fail/schema-rest-missing.cirru --check-only`
- `cargo run --bin cr -- calcit/type-fail/schema-rest-unexpected.cirru --check-only`
- `cargo run --bin cr -- calcit/type-fail/schema-kind-mismatch.cirru --check-only`
- `cargo run --bin cr -- calcit/type-fail/schema-call-arg-type-mismatch.cirru --check-only`

其中：

- 前 4 个会触发 `schema mismatch while preprocessing definition`（定义时校验）。
- `schema-call-arg-type-mismatch.cirru` 会触发基于 schema 的函数参数类型告警，并在 `--check-only` 下被当作错误处理。

## 自动化测试

这些 fixture 已接到 Rust 测试里，会随 `cargo test` 一起运行。

- schema mismatch fixtures：断言最终错误文本包含 `E_SCHEMA_DEF_MISMATCH`
- call-site arg mismatch fixture：断言产生 `W_FN_ARG_TYPE_MISMATCH`

相关测试位于 [src/bin/cr.rs](src/bin/cr.rs)。

日常单独跑这组测试时，可以直接使用：

- `yarn test-fail`

这个命令会执行 `cargo test -q --bin cr type_fail_`，专门覆盖这批 type-fail / schema-fail fixture 对应的测试。

## 当前相关 code

- `E_SCHEMA_DEF_MISMATCH`：定义与 `:schema` 的 `:kind` / `:args` / `:rest` 不匹配
- `W_FN_ARG_TYPE_MISMATCH`：用户函数调用参数类型不匹配
- `W_PROC_ARG_TYPE_MISMATCH`：内建 proc 参数类型不匹配
- `W_CORE_FN_ARG_TYPE_MISMATCH`：`calcit.core` 函数参数类型不匹配
- `W_FN_RETURN_TYPE_MISMATCH`：函数声明返回类型与函数体实际返回类型不匹配

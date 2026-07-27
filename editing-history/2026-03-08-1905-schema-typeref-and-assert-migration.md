# 本次修改记录

## 主题

- 完善 schema 类型系统：区分泛型变量与命名类型引用，支持 `'Result` 一类类型引用在 schema 中稳定往返。
- 继续推进 schema-first 迁移：清理 `calcit-core` 与若干测试文件中的 `assert-type`，将约束收敛到 `:schema` 与局部 `hint-fn`。
- 强化诊断：改进运行时参数个数错误与构建期 schema 解析报错，方便定位问题。

## 主要改动

### 1) Rust 侧类型系统收紧

- 在 `src/calcit/type_annotation.rs` 中新增命名类型引用分支，并让解析、匹配、显示、序列化都感知泛型作用域。
- `src/builtins/syntax.rs` 与 `src/builtins/records.rs` 改为按泛型上下文解析 schema，避免把命名类型误判成 type var。
- `src/codegen/gen_ir.rs`、`src/snapshot.rs` 同步适配新 schema 结构与 round-trip 测试。

### 2) schema / hint-fn 迁移

- `src/cirru/calcit-core.cirru` 中移除剩余 `assert-type` 调用点，改为顶层 `:schema` 或局部 `hint-fn`。
- `calcit/test.cirru`、`calcit/debug/check-args.cirru`、`calcit/test-fn.cirru`、`calcit/test-types.cirru` 改用更精细的函数签名与局部约束。
- 修正函数类型直写 schema 的参数形式，统一使用 `([] ...)` 表达参数列表。

### 3) 错误信息增强

- `src/runner.rs` 在参数个数不匹配时生成结构化报错，而不是直接触发内部 panic。
- `build.rs` 对 schema EDN 解析错误增加截断与整理，避免输出过长、难以定位。

## 验证

- `cargo fmt` 通过。
- `cargo clippy -- -D warnings` 通过。
- `yarn compile` 通过。
- `cargo test` 通过。
- `yarn check-all` 通过。

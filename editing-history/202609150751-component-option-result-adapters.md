# 增加 Component Option/Result adapter

- 直接复用类型推导保留的 `calcit.core/Option` / `calcit.core/Result` 名义类型；旧 `Optional<T>`、Dynamic 和用户自定义同名 Enum 不进入这条边界。
- 用一套 variant memory layout、flat join 与递归 lift/lower helper 覆盖 Option 和 Result，不为每个 payload 组合增加独立规则。
- `Unit` 结果映射为零个 Canonical ABI result；Unit 参数必须省略，避免引入伪值位置。
- Calcit definition `:tests` 覆盖表层 Option/Result/Unit 语义；Rust/Node 集成测试只覆盖 core import/export、memory layout、i64 join 和非法 discriminant trap。
- 在 #1110 合并后对齐正式 `main`，并复用 List slice 的指针对齐、精确转换与完整 memory region 校验，避免 variant 形成较弱的第二套边界。
- 当前闭包验证 `Option<Number>`、`Option<String>`、`Result<Number,String>`、`Result<Unit,String>` 与 `Result<List<Number>,String>`；后续普通 record/variant 继续复用 compound codec 顺序，不建立新 CLI。
- 验证：`cargo fmt --check`、`cargo clippy --all-targets --all-features -- -D warnings`、`cargo test`、`yarn check-all`、文档检查与 package 清单。

# schema type-fail tests and error codes

- 新增 `calcit/type-fail/` 下 schema 相关失败 fixture 的自动化测试，覆盖：
  - `:args` required arity mismatch
  - `:rest` presence mismatch
  - `:kind` mismatch
  - 基于 schema 的调用参数类型不匹配
- 在 `src/bin/cr.rs` 中补充 fixture 加载 helper，并通过 `run_check_only()` / preprocess warning 断言行为。
- 为类型/预处理相关告警补充 code：
  - `W_FN_ARG_TYPE_MISMATCH`
  - `W_PROC_ARG_TYPE_MISMATCH`
  - `W_CORE_FN_ARG_TYPE_MISMATCH`
  - `W_FN_RETURN_TYPE_MISMATCH`
- 为 schema 定义不匹配错误补充 code：
  - `E_SCHEMA_DEF_MISMATCH`
- 为 `CalcitErr` 增加 `code` 字段和 `headline()`，让 CLI 输出可携带错误码。
- 更新 `calcit/type-fail/README.md`，记录自动化测试与当前相关 code。

# calcit.cli 选项 map 类型化 schema（Rust spec 直检）

## 变更概要

- 选项 spec 收敛到 `src/builtins/cli_options.rs`（Rust 单一来源），注册时写入 `RegisteredProcDescriptor.cli_options`。
- preprocess 直接读 Rust spec 校验 options map，不再生成 defstruct / 不做 map→record 转换。
- core 中 `:args` 仍由脚本生成 inline 类型 map（供 query/docs），校验走 Rust 而非 Calcit 类型系统。

## 后续优化（Rust spec 直检）

- preprocess 热路径改用 `registered_proc_cli_options()`，不再 clone 整个 `RegisteredProcDescriptor`。
- `registered_proc_has_tag` 同样改为只读 map 查询。
- `check_cli_options_map` 与 `resolve_cli_args` 共用 `collect_validation_errors`，preprocess 侧用引用收集 map 条目，避免多余 clone。
- Registered 调用检查复用已有的 `processed_args`，去掉重复 `ys.drop_left()` 和 `alias` clone。

## 验证案例（cr eval）

| 案例 | 结果 |
|------|------|
| `:file-pth` 拼写错误 | `W_CLI_OPTION_UNKNOWN_KEY` |
| 缺少 `:file-path` | `W_CLI_OPTION_MISSING_REQUIRED` |
| `:lines \|bad` 类型错误 | `W_CLI_OPTION_TYPE_MISMATCH` |
| 正确调用 | Check passed |

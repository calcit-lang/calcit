# 0.33.0 删除 `&format-ternary-tree`

## 决策与范围

#2193 在 0.31.0 先弃用 `&format-ternary-tree`，删除原计划在 0.32.0。0.32.0 只做语法收口，删除顺延到 0.33.0。删除前对 46 个生态仓库的默认分支浅克隆重新普查，`*.cirru` 中没有调用；Explain Ternary Tree 只在 README 里说明旧 helper 不能当作树数据，项目已自己构造教学树。

## 实现要点

- 删除 `NativeFormatTernaryTree` proc、签名、失败描述、native 实现、core 定义与附带测试，以及各后端排除清单与质量基线中的对应行。
- 残留调用由 `retired_core_function_migration` 报告未知名字，给出删除版本 0.33.0 与改写方式（显示值用 `format-cirru-edn`，教学树在项目里显式构造）。
- JS 发射中专为这个 helper 做的可达性检查（`find_js_unavailable_proc`、`js_unavailable_defs`、入口依赖遍历）没有别的对象，一并删除；入口存在性检查保留为 `validate_js_entries`。`@calcit/procs` 的 `format_ternary_tree` 警告桩删除。
- 弃用报告的 CLI 测试改用项目内自定义的弃用定义。

## 验证方式

- `cargo test`、`cargo clippy --all-targets -- -D warnings`、五个分片的 `check-all` 与文档检查。
- 用 `--check-only` 确认残留调用的告警文本与 upgrade.md 一致。

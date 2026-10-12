# CLI 统一 `--format human|edn|json`（#1566 阶段 A）

关联 #1566。此前带结构化输出的叶子命令各用一套值：`test`、`analyze quality`、`cursor show/region/marks/clipboard` 只有 `human|json`，`call-graph`/`count-calls` 是 `text|json`，`effects-graph` 是 `tree|json`，`query schema` 等用 `--json` 开关，`query ns` 只有 `human|cirru`，`tree show` 没有 `--format`。

## 决策

不删除任何入口，只让所有这些叶子接受 `StructuredOutputFormat` 的三个值：

- `StructuredOutputFormat::parse_graph` 让 `text` 与 `tree` 在 call-graph、count-calls、effects-graph 上都等价于 `human`；其余未知值保持旧行为回退到 human，并在 stderr 输出 `[Warn]`，下一个非 patch 版本再改为报错（patch 版本不收紧）。
- JSON 文档统一经 `print_value` / `print_json_text` 输出，edn 由同一个 JSON 到 Cirru EDN 转换产生，两种格式语义等价；状态行在结构化模式下写 stderr。
- `--json` 开关（`query schema`、`query def`、`tree show`、`ffi export`）保持原行为，使用时 stderr 输出 `[Deprecated] ...` 一行。`query schema` 中 `--json` 等价于 `--format json`；`query def`、`tree show` 的 `--json` 仍是在 human 输出末尾追加 JSON；`ffi export --json` 等价于 `--format json`。`--json` 与显式矛盾的 `--format` 一律报错（`query schema`/`query def`/`tree show` 拒绝 edn 与 cirru，`ffi export` 只接受 json）。`query search-expr --json` 与 `cirru format` 的 JSON 参数是输入编码，不属于输出格式，不动。
- `query ns` 增加 `edn|json`（列表与单个命名空间两种 envelope），保留 `cirru`；`tree show --format edn|json` 输出被选节点的 JSON AST envelope；`query schema` 新增 `--format`。
- 根命令 `--format` 仍只属于 `--check-only` 报告，错误提示改为指向“写在子命令之后”。

## 兼容边界

patch 安全：旧值、旧开关、人类输出与退出码不变；新增的只有 `edn` 取值、`--format` 选项和 stderr 弃用提示。`analyze call-graph --diff` 仍只接受 `human`/`text`。

## 验证

`tests/format_flag_cli.rs` 覆盖各命令的 edn/json/human 输出可解析、旧拼写、`--json` 弃用提示与冲突、cursor 视图和根 `--format` 提示；`structured_output.rs` 单元测试覆盖别名与 `--json` 解析。用户可观察的语言语义没有变化，因此不新增 Calcit `:tests`；CLI 参数解析属于 Rust 边界。

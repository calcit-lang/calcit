# program-diff 读取旧类型存储的 Git 基线

#2503：升级 review 用 `analyze program-diff <rev>` 比较升级前后的 Snapshot，但 Git 基线常常还是旧的类型存储写法（EDN `:: 'StructDef` schema、`::`/`{}` 代码类型）。严格加载器拒绝这些写法（`E_LEGACY_TYPE_SYNTAX`），而历史内容无法被 `edit format` 改写，命令在分类前就失败了。

决定：Git revision 一侧是只读历史，用 `snapshot::load_historical_snapshot_data` 读取。严格加载成功就直接用；否则在内存里依次调用 `edit format` 使用的 `load_snapshot_data_for_format`、`canonicalize_snapshot_type_syntax`、`render_snapshot_content`，再把渲染结果交给严格加载器。这样得到的 Snapshot 与对历史文件运行 `edit format` 后再加载完全相同，后续的 diff、schema 方向判断与名义类型身份都不需要知道旧写法。`program-diff`（含 `--def`）和 `call-graph-diff` 的历史一侧都改用它，工作区一侧仍严格加载。

兼容边界：执行、`--check-only` 与 codegen 仍拒绝退役写法；没有新增开关、CLI 或 fix 规则。`edit format` 也不能迁移的表示（如 0.33.0 移除的 `Optional<T>`）保留原诊断报错，不伪造等价。content revision 仍是 Git blob 原文的哈希，因此只有存储写法不同时分类为 `canonical-format-only`。发生归一化时在 stderr 输出一行说明，结构化 stdout 不增加字段。

验证：`cargo test --lib historical_load`；`cargo test --test format_flag_cli program_diff`（旧基线 → format 后为 `canonical-format-only`、两 revision 模式、真实 return 放宽仍被 `--fail-on-widening` 拒绝、`Optional` 基线明确报错）。

# Markdown 文档检查的目标参数

关联 #1305。此前 `docs check-md --entry` 选择求值用的 Snapshot 文件，顶层与 `config` 的同名选项却选择 Snapshot 内的 named entry。Agent 若复用参数经验，会把文件路径误当作配置名，或把配置名误当作文件路径。

现将 `docs check-md` 的文件选择改为 `--snapshot`，默认仍是 `calcit.cirru`。旧 `--entry` 不再静默沿用不同语义，而是明确失败并提示迁移；本次不新增顶层命令，也不改变 Markdown 代码块求值和 named entry 的现有选择规则。仓库脚本与当前指南同步替换。

`tests/docs_check_md_target_cli.rs` 验证新参数能检查真实代码块、旧参数非零退出且说明迁移。仓库文档检查脚本按新参数遍历 70 个 Markdown 文件通过；定向 Rust 测试、Clippy 和格式检查通过。本改动仅完成 #1305 的目标参数子项，List 方法、Dynamic/Unknown 和冷启动指南仍需后续处理。

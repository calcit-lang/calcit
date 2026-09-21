# 将 `exec` 合并进 `eval --stdin`

0.19 CLI 入口收敛：`exec` 与 `eval` 共用相同的临时 Snapshot、依赖加载和求值路径，唯一差别是片段来源。删除顶层 `exec` 与重复分支，在 `eval` 增加显式 `--stdin` 开关；位置参数与 stdin 互斥，不传输入或传入空 stdin 会报告错误。

Agent 与运行文档统一使用 `calcit eval --stdin`，保留 `--dep` 用法。CLI 冒烟测试覆盖管道求值、互斥和空输入失败，以及旧 `exec` 的退役。该变化只涉及命令输入边界，语言语义不变，所以验证放在 CLI 集成测试而非 Calcit definition `:tests`。

同时把顶层 `ir` 的 help 与运行文档明确标为编译器诊断入口，不把它当成普通应用运行目标；`wasm` / `wasi` 仍按不同宿主契约保留。

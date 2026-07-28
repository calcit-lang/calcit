# Cursor 原生结构编辑与跨层导航

## 概要

- 新增 `cursor apply <operation>`，由 active cursor 推导 definition target 与 tree path，再复用既有 tree handler，避免 Agent 在连续操作中重复传递坐标。
- 新增 `cursor slurp-next` 与 `cursor barf-last`，通过已有 `edit mv` 完成跨 parent 的 Paredit 风格节点移动，cursor 始终跟随原先选中的 list。
- 新增 `cursor forward/backward --count N`，按 definition 的深度优先结构顺序跨 list 边界移动，并把整次多步移动记录为一条 history。
- 当顶层使用 `--cursor-after focus` 时，set、search 选中、普通导航与 history/stack 恢复会立即输出 focus preview。
- 非法 root、leaf、空 list、缺少 sibling、零步和越界操作均在写 Snapshot 前失败。

## 验证

- `cargo fmt`
- `cargo clippy -- -D warnings`
- `cargo test`
- `yarn compile`
- `yarn check-agent-interface`
- `yarn check-all`
- `cr docs check-md` 检查 Agent 指南和 tree editing 文档。
- 全局安装当前 `cr` 后，在 Respo workflow 临时副本往返执行 depth-first navigation、swap、slurp/barf；Snapshot 与原文件逐字节一致，`cr js` 成功。

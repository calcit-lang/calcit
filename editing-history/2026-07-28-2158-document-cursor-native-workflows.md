# 补充 Cursor 原生工作流文档

## 概要

- RFC 增加 `cursor apply`、`slurp-next`、`barf-last`、`forward/backward` 的命令契约、复用边界和验收项。
- Agent 快速指南说明同级导航与跨 list 深度优先导航的区别，并加入 cursor 原生 mutation 的选择建议。
- tree editing 指南补充完整命令示例、`--cursor-after focus` 的导航反馈，以及 `unwrap` 并非任意 wrapper 的严格逆操作这一边界。

## 验证

- `target/debug/cr calcit/test.cirru docs check-md docs/CalcitAgent.md --entry calcit/test.cirru`
- `target/debug/cr calcit/test.cirru docs check-md docs/run/edit-tree.md --entry calcit/test.cirru`
- `git diff --check`

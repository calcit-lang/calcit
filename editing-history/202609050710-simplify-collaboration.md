# 2026-09-05 07:10 UTC — Simplify collaboration governance / 简化协作治理

## 中文

- 删除远程 `agent-lock/*` 租约脚本、对应测试和四态 `agent:*` 标签协议。
- 将 `AGENTS.md` 中约百行的租约、heartbeat、镜像修复和通知调度规则收敛为简短的 Issue、worktree、PR、review、Actions 与发布门禁。
- 保留并强化关键约束：并行工作隔离、非发布变更必须 PR、最新 head 完成审查且 Actions 全绿、main 精确 HEAD 验证成功后才允许 tag/release。
- 将 Issue 模板改为普通实现任务模板，不再要求 agent 状态或租约。
- GitHub 状态迁移完成：88 个开放 Issue 均保留且只保留一个通用 `status:*` 状态（迁移检查点为 75 blocked、13 ready），旧四态 `agent:*` 标签已删除。
- 删除 5 个已核对的远程 `agent-lock/*` ref；删除前的 commit SHA 已保存在 #849 的治理记录中，未清理任何本地 worktree。

## English

- Remove the remote `agent-lock/*` lease script, its tests, and the four-state `agent:*` label protocol.
- Replace the long lease, heartbeat, mirror-repair, and notification rules in `AGENTS.md` with concise Issue, worktree, PR, review, Actions, and release gates.
- Preserve and strengthen the essential constraints: isolated parallel work, PRs for non-release changes, completed latest-head review with green Actions, and exact-main success before tagging or releasing.
- Make the Issue template a general implementation-task template without agent state or lease requirements.
- Complete the GitHub state migration: all 88 open Issues retain exactly one generic `status:*` state (75 blocked and 13 ready at the migration checkpoint), and remove the four obsolete `agent:*` labels.
- Delete the five verified remote `agent-lock/*` refs while preserving their commit SHAs in the #849 governance record; do not clean any local worktree.

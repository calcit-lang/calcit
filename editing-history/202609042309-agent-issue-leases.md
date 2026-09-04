# Agent Issue leases

- 将 `calcit-lang/calcit` Issues 定义为主项目和所有关联仓库工作的唯一协调状态源。
- 使用远端 `agent-lock/issue-<number>` Git ref 提供原子认领；Issue 标签与单条可更新评论只作为可见镜像。
- 租约默认 45 分钟，支持心跳、过期后的 `--force-with-lease` 原子接管，以及 `ready`、`review`、`blocked` 三种释放结果。
- Issue 表单要求预先声明跨仓库修改范围、排除项、依赖关系和验收命令，以便并行 Agent 在写入前发现范围冲突。
- 仓库只需一次 `init` 创建状态标签；后续认领、心跳和释放都会修复标签与锁不一致的常见中断状态。
- PR 创建后仍由发起 Agent 持续关注 Actions、review 和 inline comments；需要修改时可从 `agent:review` 重新认领，修复推送后再释放回 review。
- 调度采用“单写锁、多 PR 观察”：CI/bot review 等待期立即释放租约并领取下一任务，在任务检查点批量巡检待审 PR；出现可执行反馈时再安全切换回原 Issue。
- 写入 Agent 必须按 Issue 使用独立分支和独立 worktree；共享 checkout 只做认领、查询和只读协调。除纯版本号发布提交外，所有实现、CI、测试和文档改动都必须有覆盖最新提交的 PR。
- Agent ID 应包含足够长的任务/会话标识和随机后缀，避免多个任务共享短前缀时错误续租或释放彼此的锁。
- 生态 Wiki 正文只维护在独立 `calcit-lang/calcit.wiki.git`，主仓库不保存 `docs/wiki/` 副本、同步 workflow 或发布脚本；主仓库仅链接 Wiki，并保留版本化契约文档作为 source of truth。
- `release` 先原子删除权威 lock ref，再更新 Issue 标签；若标签更新中断，后续 claim 可修复无锁的陈旧 `agent:claimed` 状态，避免出现“已 review 但锁仍有效”的误导状态。

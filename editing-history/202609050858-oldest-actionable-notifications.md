# Oldest actionable notifications first / 优先处理最早可执行通知

## English

- Sort batched GitHub notifications by ascending `updated_at` and handle the oldest actionable PR first.
- Leave actionable notifications unread when another valid Issue lease owns the work; inspect them read-only and continue with the next available item.
- Permit explicit security, release-blocker, and newly failing required-check exceptions, with the reason recorded on the coordinating Issue.

## 中文

- 批量拉取 GitHub notifications 后按 `updated_at` 升序排列，优先处理最早且可执行的 PR。
- 其他有效 Issue 租约持有的工作只读检查，并保留其待处理通知未读，再继续下一个可执行项。
- 安全风险、发布阻塞与 required check 新失败可以明确越序，但要在协调 Issue 记录原因。

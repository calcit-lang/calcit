# Oldest actionable notifications first / 优先处理最早可执行通知

## English

- Apply one total order: security risk, current release blocker, newly failing required check, then other notifications; within each class sort by ascending `updated_at`, then numeric notification ID or canonical thread URL.
- Leave actionable notifications unread when another valid Issue lease owns the work; inspect them read-only and continue with the next available item.
- Record the reason when one of the first three priority classes precedes an otherwise older notification.

## 中文

- 使用唯一全序：安全风险、当前发布阻塞、required check 新失败、其他通知；每类再按 `updated_at` 升序，最后按数字 notification ID 或 canonical thread URL 排序。
- 其他有效 Issue 租约持有的工作只读检查，并保留其待处理通知未读，再继续下一个可执行项。
- 前三类优先于时间更早的普通通知时，要在协调 Issue 记录原因。

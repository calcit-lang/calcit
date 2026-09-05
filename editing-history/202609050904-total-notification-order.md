# Total notification order / 通知全序规则

## English

- Resolve competing priority exceptions before comparing notification timestamps.
- Use numeric notification ID, or canonical thread URL when no ID exists, as the stable final tie-breaker.

## 中文

- 多个优先级例外同时出现时，先确定优先级，再比较通知时间。
- 最后使用数字 notification ID；没有 ID 时使用 canonical thread URL，保证排序稳定。

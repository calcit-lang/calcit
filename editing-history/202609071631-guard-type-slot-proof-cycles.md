# Guard type-slot proof cycles / 保护 type-slot proof 环

## English

- Apply the existing relation-local type-slot guard to static proof in both
  directions, not only compatibility checks.
- Return the explicit `RecursiveTypeSlot` boundary when a slot graph re-enters
  itself, and add a regression for a two-slot cycle.

## 中文

- 将现有的关系局部 type-slot 环保护同时用于双向静态 proof，而不只用于
  compatibility 检查。
- slot 图重新进入自身时返回明确的 `RecursiveTypeSlot` boundary，并增加双 slot
  环回归。

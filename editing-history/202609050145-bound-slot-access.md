# Resolve bound slots for optional indexed access

2026-09-05 01:45 CST

## English

- Resolve already-bound type-slot chains before choosing typed optional indexed-access specialization or strict rejection.
- Preserve open-boundary behavior for unresolved and cyclic slots.
- Cover a bound List specialization and a bound unsupported Number receiver.

## 中文

- typed optional indexed access 在选择特化或 strict 拒绝之前，先解析已经绑定的 type-slot 链。
- 未解析或循环 slot 仍保持开放边界行为。
- 回归测试覆盖已绑定 List 的特化，以及已绑定 Number 接收者的拒绝。

# Component Enum adapter

中文：

- 为闭合单态 Enum 增加通用 Canonical ABI lift/lower，不再为每个 case 组合增加独立规则。
- discriminant 由规范化声明顺序决定；无、单个和多个 payload 共用递归 layout 与 flat join。
- Calcit `:tests` 覆盖直接与宿主往返语义；Rust 只补充 ABI layout、memory 边界和拒绝路径。
- generic、anonymous/open、递归、未解析 Enum 以及显式 Unit payload 在生成阶段给出稳定 schema path。

English:

- Add general Canonical ABI lift/lower adapters for closed monomorphic Enums without per-case special rules.
- Derive discriminants from normalized declaration order and share recursive layout/flat joining across zero, single, and multiple payload cases.
- Cover direct and host round trips in Calcit `:tests`; keep Rust coverage focused on ABI layout, memory boundaries, and rejection paths.
- Reject generic, anonymous/open, recursive, unresolved Enums and explicit Unit payloads during generation with stable schema paths.

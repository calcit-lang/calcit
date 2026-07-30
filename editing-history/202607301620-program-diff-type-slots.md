# Program diff 补齐 entry type slots

- `SnapshotConfigs.type_slots` 已用于默认 `:configs` 和命名 `:entries`，但 program diff 仍只枚举旧的 `init-fn`、`reload-fn`、`version` 与 `modules` 字段，导致纯 type-slot 修改被误报为无变化。
- 为字符串 map 增加稳定按 key 排序的结构化 diff，slot 标签保留 `:dispatch-op` 形式；新增或删除整个 entry 时也完整展示其 type slots。
- `Snapshot` 与 `SnapshotConfigs` 的 program diff 改为无 `..` 的完整字段解构。以后新增字段而未更新 diff 时会产生编译错误，避免同类静默遗漏。
- 回归测试覆盖默认 entry 的 slot 新增、删除、替换，命名 entry 的 slot 修改，以及新增 entry 时的 slot 详情。

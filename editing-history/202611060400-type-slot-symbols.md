# type slot 写成 symbol，未绑定或绑定 `Dynamic` 时告警

方向见 Jon 2026-11-06：用到 type slot 的地方必须设置 slot，slot 值改用 symbol；绑定成 `Dynamic` 不算已设置。0.36.x 是 Notes，0.37.0 改为错误。

- 写出：`snapshot::type_slot_value_edn` 统一 `:type-slots` 的值，具名类型写 `Edn::Symbol`（`'app.schema/Op`），opt-out 写 `'Dynamic`。`edit format`（`type_slots_to_edn`）和 `config set-type-slot`（`type_slots_as_edn`）都走它。
- 读取：`parse_snapshot_type_slots` 仍接受 string / symbol / `:dynamic`。`snapshot::collect_type_slot_spelling_hits` 从原始 EDN 找出选中 entry 的字符串绑定、`:dynamic` tag 和非 tag 的 slot 名，`cr` 加载后作为 `W_TYPE_SLOT_STRING` Notes 入队（位置 `entries.<entry>/type-slots`）。没有在 `SnapshotEntry` 上加字段，因为写法只在原始 EDN 里可见。
- 绑定：`reject_strict_unbound_type_slot_schema` 在严格模式的项目源码里仍对未绑定 slot 报 `E_UNBOUND_TYPE_SLOT`；其余情况（非严格、依赖库定义、绑定成 `Dynamic`）用 `find_open_type_slots_in_schema` 收集每个 slot，报 `W_UNBOUND_TYPE_SLOT` Notes。内置 core namespace 排除。具名类型内部的 `Dynamic` 暂不报。
- `config set-type-slot :x Dynamic` 仍可写入，但打印 warning。

fixtures 与文档里的字符串绑定改为 symbol；`snapshot` 的 round-trip 测试保留旧写法作为输入，断言输出是 symbol 且旧写法被报告。

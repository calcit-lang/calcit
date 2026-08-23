# Definition schema round-trip 修复

- 发现连续两次 `calcit edit/tree` 会把 `StructDef` / `EnumDef` schema 误读为匿名 `Enum`。
- 根因是 Snapshot 的零 payload 类型包装 `:: 'Type` 仅对 `Dynamic` 特判，其他 canonical symbol 落入普通 enum 解析。
- 统一通过 canonical type symbol parser 读取非 callable 的零 payload schema，并覆盖两次 load/save 的回归测试。
- 真实复现与影响记录在 calcit-lang/calcit#390。

# Struct / Enum 数据模型文档跟进清理

- 同步文档措辞：`tuple` / `record` 术语已迁移为 `enum` / `struct`，将仍然把当前数据模型描述为旧名的 4 处表述更新为 struct/enum 语义。
- `docs/features/polymorphism.md`：`records/tuples created from them` → `struct/enum values created from them`。
- `docs/docs-indexing.md`：category registry 中 `records, tuples, enums` → `structs, enums`。
- `docs/data/edn.md`：严格 EDN 解码不把 `ordinary tuples` 强转成 enum → `ordinary lists`，避免与已移除的 tuple 类型混淆。
- `docs/features/structs.md`（原 `records.md`）：匿名 struct 字段排序说明 `struct-backed records` → `named structs` 的字段顺序。
- 保留的合法旧名引用：迁移表（`anonymous-enums.md`、`upgrade.md`）、frontmatter alias / 旧文件名链接（`structs.md`、`anonymous-enums.md`）、`impl records`（trait 概念）、`namespace records`（快照条目）等不属于数据模型旧名。
- 验证：`cr docs check-md <file> --entry calcit/test.cirru` 结果与改动前一致（structs.md 的 11 个失败均为匿名 `%{} _` 示例在 eval 上下文无法运行的既有问题，与本次改动无关）。

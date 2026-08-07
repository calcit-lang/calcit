# Struct / Enum 数据模型文档文件重命名

- 将文档文件名与新的 struct/enum 模型对齐：`docs/features/records.md` → `docs/features/structs.md`，`docs/features/tuples.md` → `docs/features/anonymous-enums.md`。
- 同步 frontmatter id：`core/features/records` → `core/features/structs`，`core/features/tuples` → `core/features/anonymous-enums`；保留旧 alias（"record type"、"legacy tuple migration"）以维持文档检索兼容。
- 更新所有交叉链接：`docs/features.md`（`leads_to` 与 Domain modeling 链接）、`docs/features/enums.md`（See Also）、`docs/features/anonymous-enums.md`（See Also）、`docs/run/agent-advanced.md`（Anonymous Enum vs List 链接）。
- 验证：终端 grep 确认 `docs/` 下无残留 `records.md` / `tuples.md` / 旧 id；`git status` 显示 `RM` 重命名记录。RFC 为历史提案，保持原样。

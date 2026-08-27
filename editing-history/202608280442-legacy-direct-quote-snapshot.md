# Legacy direct-quote Snapshot migration

- 为 `calcit edit format` 增加与 runtime loader 隔离的一次性旧 Snapshot 解析入口。
- 早期 direct quoted namespace/definition 分别迁移为 `NsEntry` / `CodeEntry`；definition 使用明确的 `Dynamic` schema，不猜测业务类型。
- 顶层 `:configs` 迁移为 `:entries.default`，缺省 mode 为 native；未知字段以及与已有 default entry 的冲突会报错。
- formatter 输出 namespace、definition 与 configs 的迁移计数，便于审阅生成的 `calcit.cirru` diff。
- 普通 loader、检查和其他编辑命令继续严格拒绝旧结构，避免长期保留双格式运行时分支。
- 使用 `calcit-lang/calcit-paint` 的 2022 compact Snapshot 完成真实回归：迁移 3 个 namespace、8 个 definition 与 configs；严格检查随后仅剩独立的 macro schema 迁移提示。

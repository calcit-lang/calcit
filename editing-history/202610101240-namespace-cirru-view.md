# 命名空间 FileEntry 数据视图与 `edit ns` 写回

关联 #1614、#1612、#1564。

`query def --format cirru`（#1841）只覆盖单个定义。读一个命名空间仍需
`query defs` 加多次 `query def`，跨定义的修改要组合 `tree` 坐标或事务参数。
本次增加 `query ns <ns> --format cirru` 与 `edit ns <ns> --file <data>`。

第一版按 issue 中的样例输出“`ns` 形式 + 各定义代码 + `:meta <name>` 块”，
并为代码不带定义名的定义引入 `:def <name> <code>`。维护者审查后否决：
`:meta` 等写法是 Calcit 之外的新语法；一次编辑整个命名空间时，应直接从
Snapshot 中摘取该命名空间的数据，而不是出现新格式。因此改为数据视图：

- `query ns --format cirru` 输出该命名空间的 `%{} 'FileEntry` 值，复用
  `render_snapshot_content` 对每个 `:files` 条目的序列化（`Edn::from` 加
  `normalize_pipe_prefixed_leaf` 与同一 Cirru 格式化），新增
  `snapshot::render_file_entry_content`。规范格式的 Snapshot 中，输出与文件里
  该命名空间的片段逐字节相同（仅缩进不同）。当前 `origin/main` 的
  `test-traits`、`test-wasm` 与 `calcit-core` 三个文件不是规范格式（`edit format`
  会改写它们），这三个命名空间的输出因此与原文本片段不同，规范化后的副本则一致。
  逐字节复现非规范文本只能直接截取文本，不经过数据，所以没有这样做。
- `edit ns` 把输入解析为 Cirru EDN，替换原 Snapshot EDN 中该命名空间的
  `:files` 条目，再交给 `load_snapshot_data` 校验，与读取 Snapshot 文件走同一个
  loader，不另写一套 FileEntry 解析。数据中的每个字段都生效。
- 按定义比较 loader 得到的 `CodeEntry`，报告 added/changed/unchanged/removed
  与变化的字段。改动的 `:code` 经过 #1612 的 `validate_definition_shape`。定义头
  按数据描述的命名空间解析（新的 imports 加上数据中的宏），每个定义仍按其
  原有代码检查名称是否一致。loader 要求 `defmacro` 带 `Macro` schema，所以数据路径
  不再自动推导保守 schema，也不改写数据中的其他字段。
- `.$meta` 命名空间由 loader 生成，不在 Snapshot 文件里，`edit ns` 拒绝写入。

有变化时复用 `run_staged_transaction_with_options`，沿用同一份暂存、revision（含
scoped revision）、并发核对与未改 schema 原样恢复；没有变化时不重新渲染，只核对
`--expect-revision`，即使文件不是规范格式也保持字节不变。

验证：`calcit/*.cirru`、`calcit/debug/*.cirru`、`calcit/js-ffi-module` 与
`src/cirru/calcit-core.cirru` 中保存在文件里的全部 58 个命名空间读出后原样写回，
Snapshot 字节不变且报告 `changed: false`；`tests/edit_cli.rs` 覆盖单定义修改与
`edit def --overwrite` 结果一致、tests 在数据中保留、doc 修改与新增定义、删除保护、
revision 不匹配、无法解析或不合规的数据在写入前被拒绝，以及同一数据中新增宏。

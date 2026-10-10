# 命名空间 Cirru 视图与 `edit ns` 写回

关联 #1614、#1612、#1564。

`query def --format cirru`（#1841）只覆盖单个定义。读一个命名空间仍需
`query defs` 加多次 `query def`，跨定义的修改要组合 `tree` 坐标或事务参数。
本次按 issue 中确认的样例增加 `query ns <ns> --format cirru` 与
`edit ns <ns> --file <view>`，Snapshot 仍是唯一存储，视图只是投影。

视图格式：第一个表达式是完整 `ns` 形式；之后每个定义按名称排序输出代码，
有 doc 或非 Dynamic schema 时紧跟 `:meta <name>` 块。schema 使用与
`query schema` 相同的 Cirru 形式（`query_schema_cirru`），写回时先与该形式
比较，相同即视为未变化，不同才经 `parse_schema_annotation_for_write` 解析并与
原类型比较，因此读出的视图不会因 schema 表示差异产生改动。样例未覆盖
代码不以定义名作第二叶子的定义：core 中约 250 个 runtime 实现占位符和
顶层 `fn` 都属于这一类，视图写成 `:def <name> <code>`。`:meta` 与 `:def`
是 tag，不可能是定义头，因此不会与定义代码混淆。只有视图文本能解析回
完全相同的表达式序列时才输出，与 #1841 的单定义视图保持一致。

写回按定义比较：代码变化时复用 `edit def` 的规则（`validate_definition_shape`
形状检查、defexternal 简写、宏的保守 schema），并用视图中的 `ns` 形式解析
定义头；`:meta` 中缺失的字段保留原值，`:schema nil` 清除。缺失的定义需
`--allow-remove`，dry-run 同样拒绝，保证预览与提交一致。历史 Snapshot 中
`ns` 形式名称与 key 不一致时（如 `test-algebra.main`），未修改的 `ns` 形式
可以原样写回，新的 `ns` 形式必须使用正确名称。

有变化时复用 `run_staged_transaction_with_options`：同一份暂存、revision
（含 scoped revision）、并发核对与 schema 原样恢复逻辑；没有变化时不经过
重新渲染，直接核对 `--expect-revision`，即使文件不是规范格式也保持字节不变。

验证：`calcit/*.cirru`、`calcit/debug/*.cirru`、`calcit/js-ffi-module` 与
`src/cirru/calcit-core.cirru` 的全部 104 个本地命名空间读出后原样写回，
Snapshot 字节不变且报告 `changed: false`；`tests/edit_cli.rs` 覆盖单定义修改与
`edit def --overwrite` 结果一致、tests 保留、`:meta` 保留与清除、新增、删除
保护和 revision 不匹配。

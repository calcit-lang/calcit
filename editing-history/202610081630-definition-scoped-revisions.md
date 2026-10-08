# 定义粒度的事务 revision 与 Snapshot 合并驱动（#1565）

问题：transaction 的 `--expect-revision` 只接受整个 Snapshot 的 md5，任何一处修改都会让其他进行中的事务失效；多个 Agent 并行修改时只能反复重新预览。分支合并时，两边在相邻位置新增或修改定义，Git 按文本报冲突。

决策：

- Snapshot 按单元计算 fingerprint：每个定义（复用 `definition_revision`）、每个命名空间的 `ns` 声明、每个 entry、项目字段（package/about/version/verification）。loader 合成的 `<package>.$meta` 不算单元。
- transaction 报告新增 `scoped_revision`：`scope:` 后列出本事务改动单元在修改前的 fingerprint。`--expect-revision` 收到 scoped revision 时只核对这些单元，并要求重新执行后的事务不改范围外单元；否则报 `Definition conflict` 并列出单元名。整个 Snapshot 的 `md5:` revision 行为不变。
- 新增 `calcit <ours> edit merge --base --theirs`，作为 Git merge driver（`calcit %A edit merge --base %O --theirs %B`）。按单元三方合并：只有一方改的单元采用该方，双方相同保留一份，双方不同保留我方并以非零退出、列出冲突单元。结果按规范格式写回，相当于自动执行 `edit format`。Git 拥有驱动的临时文件，因此 merge 不取写锁、不创建 `.calcit` 锁文件。

兼容边界：Snapshot 文件格式与字段不变；同一 Snapshot 的写入仍由写锁串行；`edit merge` 是 `edit` 下的新子命令，未新增顶层命令。

验证：`tests/edit_cli.rs::scoped_revisions_let_transactions_on_different_definitions_commit_independently` 覆盖不同定义依次提交、同一定义冲突、范围扩大被拒绝与整体 revision 兼容；`merge_driver_merges_definitions_and_reports_same_definition_conflicts` 用真实 Git 仓库验证无改动合并字节不变、两边在同一位置新增定义（纯文本合并会冲突）可自动合并、同一定义冲突时 Git 报冲突且文件保留我方版本。

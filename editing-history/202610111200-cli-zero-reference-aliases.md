# CLI 入口收敛第二批：零引用命令

关联 #1566，延续 `202610101700-cli-consolidation-aliases.md` 的机制。维护者批准只收拢盘点中零引用（或只在本仓库文档出现）的命令；有生态使用者的项目与开放问题不在本次范围。

## 决策

- `analyze count-calls` 与 `analyze call-graph-diff` 改为 `analyze call-graph` 的 `--counts`（配 `--sort`）与 `--diff <ref>`。仍在参数解析后改写成原 analyzer 的选项结构，新旧入口共用实现，stdout 与退出码相同。两个选项互斥；`--counts` 拒绝 `--max-depth`、`--show-unused`，`--diff` 拒绝非 `text` 的 `--format`、`--show-unused`，`--sort` 要求 `--counts`。选 `--counts` 开关而不是盘点中的 `--format counts`：计数视图自己有 text/json 两种格式，放进 `--format` 会冲突。
- 计数相同的定义原来按 HashMap 顺序输出，同一命令两次运行顺序也可能不同；现在按名字作次序，使新旧入口能逐字节比较。
- 评审修正：`--sort` 只接受 `count` / `name`，并同样作用于 JSON 输出（原来 JSON 忽略它，未知值静默按 count 处理）。`call-graph` 与计数视图在 `--format json` 时把 `Analyzing call tree from:` / `Counting calls from:` 状态行写到 stderr，stdout 只有一个文档；text 格式保持原样，因为 `call-graph` 的 text 输出已有使用者。`--start` 不小于文件行数时返回错误，不再因切片越界 panic。
- `docs read-lines` 改为 `docs read --start/--lines`，沿用原选项名与默认值（0、80），迁移只需改子命令名；与标题关键词、`--full`、`--with-lines`、`--no-subheadings` 混用时报错。
- `cirru show-guide` 指向 `docs read cirru-syntax.md`，两者读同一文件；旧命令保持原样输出原文。
- `analyze js-escape`、`js-unescape` 没有替代，提示写明删除后不再提供。
- `edit rm-example` 由 `edit examples`（写入剩余示例）或 `edit ns` 覆盖；`edit ns-doc` 由 `edit ns` 写回修改了 `:ns` doc 的数据覆盖，测试比较两种写法得到的 Snapshot 相同。`edit ns` 不能放进 `edit transaction`，因此事务中的 `ns-doc` 操作需要拆成单独的 `edit ns` 步骤；事务中的 `rm-example` 改用 `edit examples` 操作。两个事务操作都输出一行弃用提示。

## 兼容边界

0.29.0 已发布，本批在 0.30.0 弃用、0.31.0 删除，删除项与迁移命令写在 `docs/run/upgrade.md` 的独立小节，不并入 0.29.0 的表格。

## 验证

`tests/deprecated_aliases_cli.rs` 比较新旧入口的 stdout、退出码与提示行；guidebook 命令通过临时 `HOME` 指向仓库 `docs/` 运行。`cli_handlers::deprecations` 单元测试覆盖改写与非法组合。

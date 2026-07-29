# 可持续迁移的 tree cursor

## 概要

- 新增 `07-28-persistent-tree-cursor-rfc.md`，将 cursor 定义为 CLI 跨调用维护的树形选择状态，而不是 snapshot 中的源码身份。
- 新增 `cr cursor set/show/clear/parent/child/next/prev`，状态以 Cirru EDN 保存到 snapshot 同目录的 `.calcit-cursor.cirru`。
- path-based tree 命令可通过 `--path @cursor` 引用 active cursor，显式 definition target 必须匹配。
- `cursor show` 在展示副本中用 `CURSOR` 包裹目标；JSON 输出仍返回真实 subtree 与独立 cursor 元数据。
- 直接 tree mutation 成功后会刷新 cursor fingerprint、preview 与 definition revision，并对插入、删除、swap、unwrap、raise、wrap、replace 等操作执行确定性 path 迁移。

## 安全边界

- cursor 文件是本地状态，已加入 `.gitignore`，不会写进 snapshot 或模块内容。
- 外部修改导致 path fingerprint 不匹配时，只允许通过唯一 fingerprint 命中自动重定位；零命中或多命中拒绝猜测。
- snapshot 已保存但 cursor 写入失败时，错误会明确说明源码 mutation 已成功，避免误认为整体回滚。
- transaction staged 子命令暂时禁用真实 cursor 写入；transaction 内 cursor 演化留到下一阶段实现。

## 验证

- cursor path transform 单元测试覆盖前方插入、前方删除、删除目标、swap 与 unwrap。
- Cursor Cirru EDN 文件 round-trip 测试。
- 临时 snapshot 集成测试覆盖源码插入后持久化 cursor 从 `@48.1` 迁移到 `@48.2`，且 preview 仍为原来的 `true` 节点。
- 手工 CLI smoke 覆盖 `cursor set/show --format json`、`tree show --path @cursor` 和 `tree insert-before --path @cursor`。
- `cargo fmt --all`
- `cargo clippy -- -D warnings`
- `cargo test`（256 lib + 147 cr）
- `yarn compile`
- `yarn check-agent-interface`（12/12）
- `yarn check-all`
- `cr docs check-md` 验证 `docs/run/edit-tree.md` 与 `docs/CalcitAgent.md`

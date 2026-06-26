# 2026-0310-1754 — snapshot namespace entry migrate to NsEntry

## 背景

snapshot 里的 `:ns` 之前复用了 `CodeEntry` 结构，导致 namespace 也带着 `:schema` 和 `:examples` 字段，语义上并不合适，而且保存时经常只是写成 `(:schema nil)`。

这次改动把 namespace 入口单独收敛到 `NsEntry`，同时保持读取旧 `CodeEntry` 写法的兼容性。

## 核心改动

### `src/snapshot.rs`

- 新增 `NsEntry { doc, code }`
- `FileInSnapShot.ns` 从 `CodeEntry` 改为 `NsEntry`
- `RawFileInSnapShot.ns` 同步改为 `NsEntry`
- 新增 `TryFrom<Edn> for NsEntry` 与 `From<NsEntry> for Edn`
- 读取 snapshot 时，`ns` 只解析 `doc` 和 `code`，兼容旧 `CodeEntry` 形状并忽略 `schema/examples`
- 移除 `validate_snapshot_schemas_for_write` 对 `ns.schema` 的校验
- `gen_meta_ns`、`create_file_from_snippet` 等内部构造统一改为写 `NsEntry`

### `build.rs`

- 嵌入式 core snapshot 解析结构新增 `NsEntry`
- build 阶段读取 `src/cirru/calcit-core.cirru` 时，`ns` 不再解析为 `CodeEntry`

### `src/detailed_snapshot.rs`

- 新增 `DetailedNsEntry { doc, code }`
- `DetailedFileInSnapshot.ns` 从 `DetailedCodeEntry` 改为 `DetailedNsEntry`
- 详细快照读取仍兼容旧 namespace record，只提取 `doc` 和 `code`

### `src/bin/cr_sync.rs`

- namespace change payload 从 `CodeEntry` 拆成 `SnapshotEntry::Ns(NsEntry)`
- definition change payload 保持 `SnapshotEntry::Def(CodeEntry)`
- detailed snapshot 写回时，namespace 统一序列化为 `NsEntry`

### `src/bin/cli_handlers/edit.rs`

- `edit add-ns` 创建新 namespace 时直接写入 `NsEntry`

## 数据文件迁移

- 批量运行 `cr edit format`，将 compact snapshot 中旧的 `:ns $ %{} :CodeEntry ... (:schema nil) :examples []` 收敛为 `:NsEntry`
- 运行 `cr-sync` 重写 detailed snapshot：
  - `demos/calcit.cirru`
  - `calcit/editor/calcit.cirru`
- `src/cirru/calcit-core.cirru` 也已更新为 `NsEntry`

## 兼容策略

1. **读取兼容**：旧 snapshot/detailed snapshot 中 `ns` 仍然可以是 `CodeEntry` record
2. **内存收敛**：加载后统一存成 `NsEntry`
3. **保存统一**：以后重新保存或 format 都写回 `NsEntry`

## 验证

- `cargo fmt` ✅
- `cargo clippy -- -D warnings` ✅
- `cargo test` ✅
- `yarn check-all` ✅

## 备注

- `demos/compact.tmp.cirru` 不是合法 snapshot，`cr edit format` 会报 EDN 解析错误，因此未纳入统一格式化
- `demos/deps.cirru` 不是当前 snapshot loader 支持的结构，因此同样未走 `edit format`
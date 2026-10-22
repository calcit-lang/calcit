# 增量更新携带定义元数据

关联 #2178。`.compact-inc.cirru` 的 `FileChangeInfo.added_defs` / `changed_defs` 原来只能是 `Map<String, Cirru>`：`cr watch` 应用新增定义时 schema 固定为 Dynamic、doc/examples/ffi/tags 为空，修改定义沿用内存中的旧元数据，只改元数据的变化无法表达，watch 状态与冷启动可能看到不同的类型合同。

决策：不另起协议，也不让 watcher 重读完整 Snapshot（大项目开销、Snapshot 可能与增量消息不同步），而是让条目接受两种载荷：`snapshot::DefinitionChange::Entry(CodeEntry)` 使用完整 Snapshot 的同一个 `CodeEntry` 加载器（schema 归一化、`:ffi` 与 `defexternal` 规范化、tests 校验），`DefinitionChange::Code(Cirru)` 为旧的裸 quote 代码。`program::apply_code_changes` 对完整条目与冷启动的 `extract_file_data` 共用 `program_def_entry_from_code_entry`（含 JS FFI 校验），因此增量结果与冷启动逐字段相同；所有命名空间先全部校验、构建（克隆受影响的命名空间后在副本上修改），再一次性提交，任何错误都不改动程序代码、运行时值或编译缓存，`cr watch` 出错提前返回时也不存在需要清理的半应用状态。完整条目同样执行冷启动的严格 `defmacro` schema 检查（`snapshot::validate_incremental_macro_schemas`），定义名沿用 `parse_snapshot_identifier_key` / `insert_snapshot_identifier`。

兼容边界：旧纯代码条目继续被接受，`changed-defs` 只替换代码并保留已加载元数据，`added-defs` 以空元数据开始（与旧行为一致），watcher 打印 code-only 条目数量。watcher 从不写回完整 Snapshot，两种载荷都不会改动其中的元数据。仓库内唯一的生产者 `calcit edit inc` 改为写出完整 `CodeEntry`；旧版 watcher 读不了新条目，会报错跳过本次更新，需要与 CLI 同版本。`:tests` 解析但运行时不保存，与冷启动相同。

缓存失效：只改元数据的条目走 `changed-defs`，本身已是失效种子，`clear_runtime_caches_for_changes` 让依赖它的编译结果与运行时值一并失效；`added-defs` 也加入种子，覆盖“新增”一个已加载定义的情况。

验证：`src/program/tests.rs` 新增与冷启动对照的新增 Fn schema/FFI 定义、只改元数据并使依赖失效（含 EDN 文本往返）、旧纯代码条目行为、非法载荷/JS FFI 拒绝且无部分应用；`tests/edit_cli.rs` 验证 `edit inc` 写出与 Snapshot 相同的 `CodeEntry`。

## English

Related to #2178. Incremental `added-defs` / `changed-defs` entries now accept either legacy bare quoted code or a full `CodeEntry` record parsed by the Snapshot loader (`snapshot::DefinitionChange`). Full entries are converted by the same helper as cold start, so schema, doc, examples, FFI and tags match a cold load; metadata-only changes are expressed as a `changed-defs` entry with unchanged code and invalidate dependents through the existing seed set (now also seeded by `added-defs`). Legacy code-only entries keep the old behaviour (changed defs keep loaded metadata, added defs start empty) and the watcher reports their count; the watcher never writes the full Snapshot. `calcit edit inc` now emits full entries, so watcher and CLI must run the same version. All namespaces are validated and built before anything is committed, so any error leaves code, runtime values and compiled caches untouched. Full entries also run the cold-start strict `defmacro` schema check, and definition names use the Snapshot identifier rules.

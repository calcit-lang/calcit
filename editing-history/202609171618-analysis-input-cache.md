# 静态分析输入缓存

## 背景

第一阶段的 `--incremental` 只复用 definition-local 报告行。每次命令仍会读取并解析主 Snapshot、所有依赖模块，再把它们
转换和合并为静态分析使用的 `Snapshot`，因此 definition 命中后仍有重复输入开销。

## 修改

- 模块加载器可返回递归访问过的 Snapshot 文件路径，普通调用继续保持原有返回类型和行为。
- `--incremental` 为普通 `check-types` / `weak-types` 保存 `.calcit/analysis-input-cache-v1.cirru`。
- 输入缓存与 definition cache 都以 Cirru EDN 为默认持久化格式；JSON 只保留给显式互操作接口。
- 输入缓存记录编译器版本、内置 core revision、主文件路径、全部源文件内容 digest、module 请求解析路径、项目 namespace
  集合和合并后的 Snapshot；符号链接或模块解析目标变化也会失效。
- Snapshot schema 在 Serde 往返时兼容 map 字段恢复为字符串键；`:ffi` 中带 Tag/Symbol key 的 Cirru EDN map
  另外以 Cirru EDN 文本保真保存，并在命中时严格恢复，避免键类型退化。
- 所有源文件未变化时直接恢复合并 Snapshot；任一源变化时全量重建输入，但 definition 缓存仍按各自 revision 判断命中。
- 模块加载失败、缓存损坏、版本/schema 不一致或写入失败都安全回退；模块缺失时不持久化输入缓存，避免后来出现的模块被遗漏。
- 现有 `data.cache` 增加独立 `input` 状态，不新增命令或并行报告入口；`preprocessing_cached` 继续明确为 `false`。
- `weak-types --schema-evidence --incremental` 继续走完整预处理路径，并把 input 状态明确报告为 `bypassed`。

## 验证

- CLI 集成测试覆盖两个输入源的 cold/warm、只修改依赖源、只修改主源、输入缓存损坏与 definition 缓存损坏。
- fixture 包含 `:ffi` 元数据，确认 input warm 后 definition revision 不漂移；单元测试覆盖泛型 `Ref<Fn>` schema 的 JSON 往返。
- 测试继续比较完整扫描、incremental cold 与 warm 的业务 envelope，排除 cache 元数据后语义一致。
- 对 Respo 的 318 个定义做真实项目回归：第二次扫描 318/318 命中，revision、覆盖统计和完整业务 envelope 与 cold 相同。
- debug 构建下对 25 个 Snapshot 输入的仓库测试集合抽样：完整扫描约 0.11 秒，input warm 约 0.08 秒；缓存文件约 583 KiB。

## 边界

输入缓存仍是进程间的解析/转换复用，不序列化编译器全局状态、macro 展开或严格预处理结果。后续只有在反向依赖图能证明
schema/import/type-slot 失效范围后，才扩大到 compiled definition 层。

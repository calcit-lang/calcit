# Definition 级增量分析缓存

## 背景

Agent 驱动的类型迁移会反复执行 edit/analyze。`check-types` 与普通 `weak-types` 的每个 definition inventory
彼此独立，但此前每次命令都会重新扫描全部选中 definition，无法复用未变化结果。

## 修改

- 在既有 `analyze check-types` 与 `analyze weak-types` 增加 opt-in `--incremental`，不新增顶层工具入口。
- 在项目 `.calcit/analysis-cache-v1.json` 保存按 definition revision 分离的 coverage 与 weak-type rows。
- 缓存上下文包含 Calcit 版本、活动 entry、type slots、feature policy 与 target；definition revision 覆盖持久化
  schema、code、tests、examples、tags、doc 与 FFI metadata。
- JSON/Cirru EDN 的 `data.cache` 和 human 摘要报告 hit、miss、状态与失效原因；损坏或不可写缓存安全回退。
- 新增或改变一个 definition 时只重算对应条目，删除的 definition 会从缓存清理；冷路径与 warm 路径的业务报告一致。

## 边界

- 本阶段只缓存无需预处理的 definition-local inventory。
- `schema-evidence` 的编译器/call-site 证据仍重新计算；`dynamic-methods`、`deprecated`、`quality` 与严格预处理
  尚未使用此缓存。
- 本地缓存不是类型正确性证明，CI 与最终验收继续保留无缓存冷检查。
- 后续扩大缓存前，必须用共享调用图证明 schema、namespace import 与 type-slot 变化的反向失效范围。

## 验证

- CLI 集成测试覆盖 cold、warm、单 definition 新增后的 partial hit、跨 analyzer payload miss、损坏缓存回退。
- 测试移除 cache metadata 后比较 cold/warm envelope，确保缓存不改变分析语义。

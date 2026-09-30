# 泛型回调与 Enum 返回类型的推断修复

在 TopixIM/diary 的迁移中，`filter-map-kv` 已声明 `Map<K,V>` 输入与 `Fn(K,V) -> MapEntryDecision<R,S>` 回调，但编译器未将前一个实参的 `K,V` 先传给后续 inline 回调。回调宏生成函数返回的具名泛型 Enum 又在 schema 序列化时丢掉 `R,S`，导致最终 Map 的键值类型无法继续约束后续写入。

本次在普通泛型调用的参数顺序处理中复用现有类型绑定关系，不新增针对 `filter-map-kv` 的专门检测规则；同时使应用后的 Enum 类型在 Calcit 与 Cirru EDN schema 表示中保留实参。显式 Dynamic 输入依然保持开放，不由后续回调凭空收窄。

回归用例包括 Calcit core definition 的 `:tests`、独立的严格编译失败 fixture，以及仅针对 schema 序列化细节的 Rust 单元测试。当前案例不说明 Diary 的所有类型迁移补丁都是编译器缺陷：外部持久化数据、Recollect patch 与 UI 投影边界仍需要各自的显式适配。

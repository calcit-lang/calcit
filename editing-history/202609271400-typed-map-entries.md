# 具名 Map 条目保留键值类型

## 背景

`&map:to-list` 的运行时结果是 `[key value]`，而列表成员只有一个统一的类型槽。
`Map<K,V>` 在这里失去键和值各自的类型，旧项目将解构后的值送入具名 Struct 参数时会触发
`E_DYNAMIC_NOMINAL_ARGUMENT`。不能仅修改该原语的类型标注：把异构二元列表伪装成
`List<K>` 或 `List<V>` 都不正确。

## 方案

在 core 层提供泛型 `MapEntry<K,V>` Struct 和 `map-entries` 函数；实现复用现有
`map-list-kv`，不增加预处理器特判，也不改变旧的动态列表 API。使用 `:key`、`:value`
字段代替位置解构，便于类型推断跨越排序和索引映射。

## 验证与边界

- `map-entries` 的定义级 `:tests` 覆盖类型参数保留，以及排序、索引映射后值类型仍为
  `Number`。
- 在 `pudica-schedule` 的临时 Snapshot 中复现旧 `comp-todolist` 对
  `app.schema/Task` 的 `E_DYNAMIC_NOMINAL_ARGUMENT`；增加独立的迁移探针后，
  `Map<String,Task>` 经 `map-entries`、`sort` 与 `map-indexed` 传给 `comp-task`，
  `analyze check-public` 的三个定义中仅旧定义仍失败。
- 泛型容器无法自动给复杂比较函数的内部参数补全类型时，应在局部 `fn` 写 `hint-fn`
  参数类型。真实消费者尚未改动，此次不声称旧业务代码已迁移或 issue 已解决。

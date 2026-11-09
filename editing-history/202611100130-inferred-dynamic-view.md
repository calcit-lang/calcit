# weak-types inferred-dynamic 视图 (#2650)

- 新增 `analyze weak-types --only inferred-dynamic`：编译所选定义，读编译结果里的 `&let` 绑定和嵌套 lambda 参数，列出类型为 `Dynamic` / `Option Dynamic` 的位置。
- 来源粗分为 dynamic-source / map-field / host-value / inference-failure；summary 给 by_binding、by_source 和未编译定义数。
- 做成视图而不是 match kind：它需要编译，源码扫描的 kind 不编译。
- 在 respo.calcit 上试跑：55 处（27 let、28 param），其中 18 处 inference-failure，多为事件回调 `(e d!)` 和直接调用的 let lambda。

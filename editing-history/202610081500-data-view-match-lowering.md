# match (data-view x) 降级为谓词链

PR #1765 引入 Data/data-view 后，`match (data-view x)` 每次先走 data-view 里的十三路 cond，再构造 Data 值，最后按变体查表。Release 构建下 200k 次分类（8 类混合输入）的耗时是同形谓词 cond 的 2.6 倍（native）和 3.8 倍（JS）。

预处理在 `try_lower_data_view_match` 中识别 `match` 的被匹配值是 core `data-view` 的单参数调用（先解析 head，局部或模块遮蔽时不降级），把它改写成 `&let` + `if` 谓词链：只测试有分支的变体，payload 用与 `defenum Data` 一致的类型绑定为带类型的局部，不构造 Data。降级发生在预处理，所以 native、JS 和 WASM 共用同一份语义，不需要各后端各写一条快速路径。

不降级的形状回到 enum 路径，由原有诊断报告：未知变体、元数、重复分支、`_` 不在最后、`:other` 与 `_` 同时出现、`:other` 分支之前没有覆盖其余所有变体。无 wildcard 且缺变体时发出与 enum 路径相同格式的非穷尽警告，运行时落到 raise。`enum?`/`struct?` 对类型定义会抛错，所以这两个变体的测试先排除 EnumDef/StructDef，与 data-view 把类型定义归入 `:other` 保持一致。

payload 绑定使用声明类型而不是被匹配值的静态类型：被匹配值已有精确类型（例如 `List<Number>`）时，绑定仍是 `List<Dynamic>`，check-known-assertion 里“开放列表元素进入具体 Struct 字段”的拒绝用例保持不变。该映射镜像 core 的 `Data` 声明，修改 `Data` 时需同步 `data_view_payload_type`。

降级后同一基准下 native 约为谓词 cond 的 1.1–1.2 倍（降级前 2.6 倍），JS 与谓词 cond 持平（降级前 3.8 倍）。

验证：definition :tests 新增降级与 enum 路径逐值对比（十八个样本，包含类型定义和 Buffer）；既有 data-view 测试、core unit、check-known-assertion（含拒绝用例的 native/JS）通过。

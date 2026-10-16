# Option→nil 源码只读视图

迁移后的 Respo 与 Recollect 保留了历史 nil-or-T 边界，严格类型检查通过并不表示这些
容器已经收窄。开放 payload 可能是 `Some nil`，复制或提前求值 fallback 会改变语义，
因此本次交付源码定位视图，不生成自动改写，也不新增诊断或预算。

入口复用 `analyze weak-types --only option-nil-roundtrip`，沿用 namespace/dependency
scope、源码坐标、human/EDN/显式 JSON 与 summary-only。只扫描定义 `:code` 的两种
方法顺序和显式 None→nil match 分支；不把 quoted data 当作运行程序，保留活动 unquote。
已知其他 nominal 类型上的同形写法排除，缺少证据的候选保持可见并标记 unknown。

输入类型直接复用 `query type-at` 的预处理 trace 与 lexical lookup，不从字符串名称
猜全局 schema，也不把 lowering 后恰好同坐标的子节点冒充原调用。直接消费者表示源码
父节点的 path/head/child index，可能是绑定或返回位置，不宣称完整 dataflow/callee 证明。
报告不会写 Snapshot 或运行应用入口；默认 weak-types、strict 和语言语义均不变。

语言侧用 definition `:tests` 展示 None、Some nil、false、零、空字符串与 Unicode 的
原行为，并通过既有 native/JS runner 回放；Rust 仅验证 CLI envelope、路径、三种格式、
scope、unknown、只读和不执行入口。真实 Respo/Recollect 额外核对位置、输入类型、检查
前后结果和 Snapshot 字节。强制诊断与容器合同改进仍按后续规划分别实施。

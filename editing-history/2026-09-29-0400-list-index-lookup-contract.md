# List 位置读取的类型契约

`List<Number>` 的 `.get/.nth` 原先分别指向泛化的 `get/nth`。编译器已有针对 List `get` 的 receiver-specialized `Option<T>` 契约及 typed optional-access lowering，因此 `.get` 查询为 proven；`.nth` 仍因通用 Dynamic schema 标为 open。运行时两者对 Number 索引都沿 `nth` 路径返回 `Option`，越界为 `none`；`get` 前缀函数还处理 Map、开放 Struct 等其他接收者，不适合全局收窄。

本批只将 List `.nth` 指向既有的 `get`，让 `.get/.nth` 共享同一类型证明与 lowering。无需增加一层 helper，也不复制索引检查规则；Map/String/Enum 的方法表和前缀函数保持原状。定义级 `:tests` 覆盖重复值、空表及越界；Agent 查询静态核对两个方法均 proven 且返回 `Option<T>`。运行行为由 native core 测试与 core WASM export 验证，本批未针对该调用路径单独验证 JavaScript 或 WASI Component。

`.get` 是 List 位置读取的推荐名。是否把 `.nth` 改写为 `.get` 留给单独的、以具体 List receiver 和同实现方法契约为前提的显式 fix；本批不做按文本的全局替换，也不混淆 `find`、`find-index` 与 `index-of`。

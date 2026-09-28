# List 分隔元素的命名

## 决策

List `join` 在元素之间插入一个值并返回 List；`join-str` 则渲染为 String，两者语义不同。新增首选名称 `intersperse` / `.intersperse`，契约仍为 `(List<T>, T) -> List<T>`。迁移期间保留前缀 `join`、`.join` 与 `join-str`。List 的新旧方法都指向 `calcit.core/intersperse`；新函数调用现有 `join` 实现，因此空列表、单元素、重复值和顺序行为不变。

## 改写边界

显式规则 `core-list-intersperse-v1` 与 seeded fold 共用简短的 List 方法别名 planner。只有完整的 `.join separator` 调用、具体 List 接收者、两种方法契约均已证明且相同、实现定义一致时，才自动改写。前缀 `join`、`join-str`、quoted data、未知 macro 上下文及附带的 tests/examples 不自动改写。规则不进入已发布的 preset，沿用现有 revision 防护和 staged validation。

共用 planner 保留 fold 规则原有的机器证据字段，同时为两条规则增加通用的同实现证据字段。

## 验证

定义级 Calcit `:tests` 覆盖空 List、单元素、重复值、顺序、新方法与保留的 `.join` 别名，4/4 通过。CLI 集成测试覆盖受证明预览/应用、语义保持、过期 revision 拒绝与幂等。当前源码的 native、生成的 JS、实际实例化的 WASM 都返回预期结果；完整 `cargo test`、`cargo clippy --all-targets -- -D warnings`、`yarn check-all`、Markdown 检查均通过。Respo 真实消费者的严格检查为 172/172 定义通过、0 diagnostics。

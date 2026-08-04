# Nil reduction foundation

## 修改概要

- 将内建过程的末尾可省略参数改为独立 arity 元数据，不再把参数值类型写成 `Optional<T>`；
- proc 参数检查保留 `Optional<T>` 的 nullable 值语义，不再为已提供参数自动剥离 Optional；
- 修正 `parse-float` 与 `get-env` 的底层返回签名，使其覆盖实际 nil 返回；
- 增加系统性减少 nil 的分阶段 RFC，明确 Unit、Optional、Option、Result 和可省略参数的边界；
- 保留内部 List/Map 低层查询的 Dynamic/专用推断兼容点，避免在缺少非空集合证据时用 `unsafe-coerce` 抹掉泛型信息。
- 扩展 `analyze weak-types` 的 nil 审计：只在可证明的返回位置使用声明契约，区分 `declared-unit`、`declared-optional` 和 `unresolved`，并输出 `W_NIL_TYPE_DEBT`。
- 修正 `calcit.core/optionally` 的 schema，由错误的 `T -> Optional<T>` 改为实际语义 `Optional<T> -> Option<T>`，并补充 `%some`/`%none` 示例与 schema 回归测试。

## 知识点

- nullable value 与 omitted argument 是正交语义，不能共用 `Optional<T>`；
- 修正底层查询返回类型会立即暴露 core 宏对非空 List 的隐含前提；在类型系统能表达 guard clause 终止和 NonEmptyList 证据前，批量强制转换不是安全迁移；
- 高层 `first`、`nth`、`get` schema 已能向 typed code 暴露 Optional，低层 `&list:*` / `&map:*` 兼容点应单独审计和收缩；
- `parse-float` 的 core schema 已是 Optional，但 Rust proc 签名仍曾声明 Number，说明重复契约需要持续做一致性检查。
- nil 合理性不能仅由函数返回类型向整个函数体传播；`do` 的中间项仍可能是遗留 sentinel，只有真实返回位置可以安全继承 Unit/Optional 契约。
- Optional 到 Option 的桥接必须保留泛型关联；将返回值继续标成 Optional 会让 nominal Option 的分支穷尽与方法推断全部失效。

## 验证

- `cargo fmt`
- `cargo clippy -- -D warnings`
- `cargo test`
- `yarn compile`
- `yarn check-all`
- `yarn check-agent-interface`
- `cr calcit/test.cirru analyze check-examples --ns calcit.core --def optionally`
- `cr calcit/test.cirru eval "let ((x (optionally (parse-float |1)))) (assert-type x (:: 'Option 'Number)) , x"`
- `cr ~/.config/calcit/modules/respo.calcit/calcit.cirru analyze check-types --summary-only`
- `target/debug/cr ~/.config/calcit/modules/respo.calcit/calcit.cirru analyze weak-types --only code-nil --intent unresolved,declared-optional --deps --summary-only --format json`

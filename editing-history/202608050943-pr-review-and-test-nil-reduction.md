# PR review and test nil reduction

## 修改概要

- 处理 PR #297 的四条有效 review：`optionally` 使用 nominal `%some`/`%none`，schema 测试绑定同一类型变量，单表达式 `do` 识别返回位置，`analyze.weak-types` 协议升级到 v2；
- 为 `optionally` 增加 enum prototype 示例，避免结构相等掩盖 nominal 身份丢失；
- 将 `test-enum.main/unwrap-maybe` 从 `Optional<T>` 迁移为 `Option<T>`，测试同步使用 `%some`/`%none`；
- 将十个空 reload 和五个 Unit callback 的裸 nil 改为显式 `(;nil)`，保留专门验证 nil/Optional 兼容行为的用例；
- 测试代码合计减少 17 个作为默认返回或缺失 sentinel 的 nil 字面量。

## 知识点

- 普通 `:: :some` / `:: :none` 只构造结构 tuple，不会附着 `Option` enum prototype；值相等不足以验证 nominal 身份；
- 泛型桥接测试必须验证输入输出共享同一个 TypeVar，否则 `Optional<T> -> Option<U>` 也会误通过；
- `do` 的第一个 child 是操作符，单表达式返回项位于 index 1，多表达式仍只有最后一项是返回位置；
- 机器协议中的封闭枚举新增值属于破坏性变化，需要升级 schema version，而不是让 v1 消费者静默遇到未知值；
- 无业务值返回应显式走 Unit；测试缺失值应优先使用 Option，只有验证兼容边界时保留 raw nil。

## 验证

- `cargo fmt`
- `cargo clippy -- -D warnings`
- `cargo test`
- `yarn compile`
- `yarn check-all`
- `yarn check-agent-interface`
- `cr calcit/test.cirru analyze check-examples --ns calcit.core --def optionally`
- `cr calcit/test-enum.cirru`
- Respo type coverage 与 nil debt JSON 回归

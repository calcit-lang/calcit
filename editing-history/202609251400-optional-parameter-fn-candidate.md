# 可选参数迁移的完整 Fn 候选

## 决策

`optional-parameters-v1` 此前只给出单个参数的 `Option<T>` 提示，维护者仍需手工重建整条 Fn 声明。现在仅从已有、完整且可序列化的 Fn schema 构造 `candidate_fn_schema_edn`：保留非可选参数、返回值、泛型和 feature，只把明确的旧 `?` 尾参数改为 nominal `Option<T>`。类型开放（包括容器嵌套、原参数、返回及 rest）、命名引用不能解析为确定 nominal struct/enum、或参数数量不匹配时不猜测完整候选。预览额外区分遗漏、显式 `nil` 与显式 `false`。

## 安全边界与验证

候选是只读证据，不是写回操作。函数体的存在判断、所有调用者、macro、外部消费者及求值/失败语义尚未证明；规则继续 `needs-review`，`--apply` 继续拒绝，也不进入默认 preset。CLI 协议测试覆盖无声明、完整声明、多个尾参数、保留泛型与 feature、EDN 可解析性和 Snapshot 不变。后续的真正事务仍须满足 #1286 的原子验证与真实项目验收。

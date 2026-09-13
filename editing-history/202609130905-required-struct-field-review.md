# 补齐必填 Struct 字段迁移的安全矩阵

## Review 结论

复杂 receiver 的原测试只比较字段结果，无法识别迁移是否重复求值。Option、有限联合和 macro origin 也需要作为 `required-struct-field-v1` 自身的 must-not-rewrite 边界，而不能由其他迁移规则的相似 fixture 间接代替。

## 调整

- 增加可观察 atom 计数：`make-fix-person` 每次调用递增计数，迁移后的 Calcit test 清零后执行字段读取，再断言 receiver 恰好求值一次。
- must-not-rewrite 矩阵补齐 `Option<FixPerson>`、具名 enum 表示的有限联合，以及经 `->` macro 展开后才产生三参数 `get` 的歧义 origin；三者 preview 都必须返回 `changed: false` 与零 suggestions。
- Calcit 当前不引入通用 union type，因此有限联合 fixture 使用现有 nominal enum，而不是为了测试新增类型机制。
- definition `:tests` 保留每种边界的用户可读调用；Rust 集成测试只验证 CLI suggestion 数量、`changed` 状态、apply 后测试执行和幂等边界。

## 验证

- `cargo test --test fix_cli`：7/7。
- `node scripts/check-agent-interface.mjs`：34/34。
- `cargo clippy --all-targets --all-features -- -D warnings`：通过。
- `yarn check-all`：通过。

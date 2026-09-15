# 处理 Option/Result review

- 将 `none`、`some`、`ok`、`err` 纳入 WASM 始终收集的 builtin tag，确保 schema 使用 Option/Result 但函数体没有构造器字面量时仍可确定生成 adapter。
- 在 discriminant 写入一字节 canonical variant memory 之前验证其只能为 `0` 或 `1`，避免 `256` 等值截断后让分支与 payload 不一致。
- 增加无构造器 tag seed 单元测试，并把 Node 边界回归扩展到会发生低字节截断的非法 discriminant。

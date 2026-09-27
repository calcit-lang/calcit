# 公开 API 的角色命名约定

## 依据

Calcit 同时保留 `%some`、`option:unwrap` 与 `&str:includes?` 等历史入口，单看名字容易混淆构造、方法和内部实现。0.25.0 已提供 `Option :some value` / `Result :ok value` 直接构造和相应迁移规则，现有 `docs/features/api-roles.md` 已给出角色导览，但缺少可作为后续改名判据的命名矩阵及明确的旧写法对照。

## 决策

公开源码的首选形态按角色区分：名义类型用 PascalCase，variant 用 tag；已知接收者的操作用 `.method` 并保留 kebab-case 与有意义的 `?` / `!`；无自然接收者的操作使用模块函数。`%` 构造 helper、`option:*` helper 和 `&str:*` primitive 仍可能是 core 或 backend 的实现名，但不再并列推荐给应用和 Agent。Rust 的参考点是角色可预测，而不是机械复制 `snake_case` 或 `Type::method`。

优先迁移可以由类型与源码解析证明的公开调用；内部函数名的实际调整必须以消除重复逻辑或真实错误为依据，避免为统一拼写重做 lowering。动态 prototype、局部遮蔽、宏边界和把旧 helper 当函数值使用的情况不作盲目自动替换。

## 验证与下一步

`docs/features/api-roles.md` 给出四组代表性迁移对照，区分 String 的 `.contains?`（索引）和 `.includes?`（子串），并说明 `query type` 中实现路径不是推荐调用。用 `calcit docs check-md` 验证代码示例；后续通过 #1427 的小批次 PR 对真实消费者、Calcit `:tests`、JS/native/WASI 和幂等 source fix 逐项验证。

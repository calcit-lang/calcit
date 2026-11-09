# core 签名的内建规则标记 `(:open :intrinsic)`（#2649）

## 决策

- calcit.core 中按接收者种类分派的函数（`filter`、`first`、`get`、`assoc`、`each`、`get-in` 等 25 个）的类型由检查器内建规则决定，泛型和 trait 约束写不出“接收者是 List 时返回 List T、是 Map 时返回 Map K V”这种关系。它们的 schema 保留运行时接受的开放形状，加 `(:open :intrinsic)` 说明原因，而不是新增一套签名语法。
- `:intrinsic` 是 `FnOpenReason` 的第四种取值，和 `:reflect` / `:decode` / `:host` 一样不参与类型关系。项目代码写了它报 `E_OPEN_WITHOUT_REASON`（理由无效），不新增诊断码；`W_OPEN_FAMILY` 汇总把它当作没有理由。
- 规则摘要只有一份：`src/runner/preprocess/intrinsic_rules.rs`。`query peek` / `query schema` 读它显示 “Checker rules”（JSON 为 `data.checker_rules`）；单元测试保证带标记的 core 定义和摘要表一一对应。
- 其余 16 个 partial 定义按语义标 `:reflect`（`str`、`data-view`、`&get-raw` 等）、`:decode`（`try-parse-json` 等）或 `:host`（`ffi:response`、`ffi:task`）。`map-kv` 不标，0.41.0 删除（#2655）。
- `analyze weak-types` 新增 intent `intentional-open`：任何带 `(:open …)` 的 `Fn` 签名内部的 schema Dynamic 归入此类，不计 `unresolved`、不产生 `W_SCHEMA_DYNAMIC`、不让 coverage 降为 partial。理由写在内层回调签名上时只覆盖该回调。
- 冗余 `hint-fn`：上下文把回调结果留成 Dynamic（如 `each`）时，与 callee 自有的结果变量同样处理，只有函数体自己推出相同结果时才算冗余。

## 兼容边界

- 类型检查行为不变；core API 合同（`scripts/core-api-contract.mjs`）比较签名时去掉 `:open`，冻结基线无需迁移条目。
- 项目里已有的 `(:open :reflect|:decode|:host)` 位置在 weak-types 中从 `unresolved` 移到 `intentional-open`，计数只降不升。

## 验证

- `cargo test --lib -- intrinsic open_famil open_without`、`cargo test --bin calcit -- reasoned_open`、`cargo test --test fix_cli -- redundant_hint`。
- `node scripts/core-api-contract.mjs`、`node --test scripts/core-api-contract.test.mjs`、`node scripts/core-dynamic-classification.mjs --check`、core `test --tag unit`。
- `analyze weak-types --ns calcit.core --deps --only coverage --coverage-level partial`：partial 函数从 42 个降到 1 个（`map-kv`）。

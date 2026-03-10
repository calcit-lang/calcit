# assert-type/assert-traits 可组合语义与检查路径修复

## 背景
- `assert-type`/`assert-traits` 之前在 runtime 路径不完整，且 `assert-type` 不可组合。
- 在补 runtime 之后出现过 `yarn check-all` 栈溢出，需要在可组合性与稳定性间做执行路径分流。

## 关键调整
- `assert-type` 通过时返回原值，失败时报错，可嵌套表达式。
- `assert-traits` 接入 syntax runtime，支持多个 trait 断言并返回原值。
- preprocess 分流：
  - local 目标：保留静态类型注入/细化（避免破坏加载路径）；
  - expression 目标：保留到 runtime 断言，保证组合性。
- 补齐 `if` 返回类型推断与方法参数泛型绑定匹配。

## 验证
- 新增/更新单测通过（runtime + preprocess 路径）。
- `cargo test` 通过。
- `yarn check-all` 通过（EXIT 0）。

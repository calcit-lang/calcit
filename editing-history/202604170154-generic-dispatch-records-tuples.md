# Extend generic assoc/nth/get/count/contains?/empty?/first to records & tuples

## 背景

Recollect 升级到 `defstruct` + `%{}` 构造记录后，在 JS 运行时调用 `assoc` /
`nth` / `get` 等会 fallback 到 `.method`，再经过 `invoke_method` 在
`structRef.impls` 中查找；但默认构造的 struct 并没有自动挂上 core 的
`:assoc` / `:nth` / `:get` impls，导致 `Error: No implementation for ... to
lookup .assoc` 等。

Rust 解释器路径能自动解析到 `&record:*` 内置 proc，所以 `cr --entry test`
能跑过，但 JS 输出（`cr js` + Node）跑不过。

## 改动

在 `src/cirru/calcit-core.cirru` 的若干 generic 分发函数里显式增加
`record?` / `tuple?` 分支，直接路由到内置 proc，不再依赖 method lookup：

| 函数        | 新增分支                                                 |
| ----------- | -------------------------------------------------------- |
| `assoc`     | `record? -> &record:assoc`, `tuple? -> &tuple:assoc`      |
| `nth`       | `tuple? -> &tuple:nth`, `record? -> &record:nth`          |
| `get`       | `tuple? -> &tuple:nth`, `record? -> &record:get`          |
| `count`     | `tuple? -> &tuple:count`, `record? -> &record:count`      |
| `contains?` | `record? -> &record:contains?`                           |
| `empty?`    | `record? -> (&= 0 (&record:count x))`, tuple 同理         |
| `first`     | `tuple? -> (&tuple:nth x 0)`                             |

这几处与“运行时按 `type-of` 分支”的方向是一致的——在当前 Calcit 语义下
records / tuples 是独立 kind，本来就应该在 generic 分发里显式处理，而不是
掉到“最后一条 method lookup” 的兜底里。

与此前添加的 **编译期单态化**（`try_specialize_polymorphic_call`）是互补的：
当静态类型已知时在预处理阶段直接改写成 proc 调用；静态类型未知时，运行时
分支也能正确分发到 `&record:*` / `&tuple:*`，而不再需要结构实现端配合
impls。

## 验证

- `cargo fmt && cargo clippy --release -- -D warnings` ✓
- `cargo test --release` ✓（179 + 67 全过）
- `yarn check-all` ✓
- recollect 全套：
  - `cr --entry test` ✓
  - `cr --entry test js && env=ci node test.mjs` ✓（此前 `.assoc` 报错修复）
  - `cr js && yarn vite build --base=./` ✓

## 后续

随着 Calcit 继续演进，这些 chain-of-if 分发仍然是临时形态。更彻底的做法是
在预处理阶段通过类型推断消除所有 generic 分发 defn，让核心库只暴露
`&list:*` / `&map:*` / `&record:*` 等单态 proc —— 见下一步语言简化计划。

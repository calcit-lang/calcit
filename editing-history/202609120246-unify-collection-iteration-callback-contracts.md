# 统一集合迭代 callback contract

## 背景

`js-object` 宏的 rest 参数已能推断为 `List<List<Syntax>>`，但 `every?` 的 receiver 专门化只发生在最终参数检查阶段。inline callback 在更早的函数体预处理阶段仍收到未实例化的 `T`，经 schema hint 往返后成为 nominal `TypeRef("T")`，最终与 `List<Syntax>` 产生伪冲突。

## 修改

- 把 `any?`、`every?` 和 `each` 纳入已有的 shared checked-call contract，让 receiver 成员类型在 callback 预处理、参数检查与返回推断之间复用。
- 保留 Syntax collection 的开放 callback 边界；Map 迭代继续只承诺异构 `List<Dynamic>` pair 形状。
- 删除 `type_checking` 中重复的 collection iteration 专门化函数，避免两套规则在不同阶段漂移。
- 增加 List、Syntax、Map 与 Dynamic receiver 回归覆盖，并补充中文类型指南。

## 验证

- `cargo test --lib -- --test-threads=1`
- `cargo test --bin calcit -- --test-threads=1`
- `cargo clippy --all-targets -- -D warnings`
- `cargo run --bin calcit -- docs check-md docs/type-guidance.md --entry calcit/test.cirru --failures-only`
- `yarn check-all`
- 当前 core 上严格检查最小 `js-object` 成功；错误 predicate 仍报告 `Number -> Bool` 不匹配。
- 在 js-ffi detached 临时副本中，将 `create-buffer` 的 `&js-object` workaround 改回 `js-object` 后，browser strict check 与 JS codegen 均成功。

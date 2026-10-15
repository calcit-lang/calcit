# Optional 与 JsNullish 合并为带来源的 Nullable（#1554、#2073）

## 依据与设计

`CalcitTypeAnnotation` 里旧 `Optional<T>` 与 `JsNullish<T>` 是两个平行变体，兼容判定、证明、推断与
codegen 中有大量只差变体名的成对分支。按 #1554 的方向（维护者选择方案 1），两者合并为
`Nullable(T, NullableOrigin)`，`NullableOrigin` 取 `Calcit`（旧 Optional）或 `JsHost`（JS FFI null/undefined）。

- 两种来源仍互不证明：同来源才比较 payload，跨来源保持原先的 mismatch / boundary 结果；
  不把 Optional 全局等同于 JsNullish，也不改 Option、诊断文本或 schema 写法。
- 只差来源的分支合并为一个分支；需要同来源的多选模式（`List | Set | Nullable ...`）保留显式的两种来源。
- `Optional` 本身的退场仍按收紧 nil 的计划在下一个非 patch 版本处理，本次不删除 schema 写法。

#2073：带 `recur` 的函数用 exit 合并类型做返回检查时，`if` 分支合并出的 `string?`（Calcit 来源）
无法证明声明的 `JsNullish<String>`，而不带 recur 时按分支逐个证明可以通过。现在对带 recur 的函数，
在合并类型证明失败后按分支证明每个值出口：recur 出口只需参数合同成立，且至少要有一个值出口。
错误的 recur 实参、错误 payload 与只有 recur 的函数仍被原有检查拒绝。

## 验证

- 合并后 `cargo test` 全部通过，行为不变；`Optional`/`JsNullish` 引用由 284 处降为 225 处，match 分支由 105 降为 91。
- `tests/fixtures/def-value-schema.cirru` 新增 `recursive-nullish-exit` 的附带测试（tag `recursive-nullable-exit`），
  在 native/JS 回放，并加入 `scripts/check-known-assertion.mjs` 已有的回放序列。
- `cargo test`、`yarn check-all`、`cargo clippy -- -D warnings`。

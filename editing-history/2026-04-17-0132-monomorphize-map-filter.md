# Monomorphize `map` / `filter` at Compile Time

## 背景

`try_specialize_polymorphic_call`（`src/runner/preprocess/mod.rs`）原本只能把
`count`、`empty?`、`first`、`rest`、`nth`、`get`、`assoc`、`contains?` 等
**built-in 多态 proc** 特化成 native proc（`NativeListCount` / `NativeMapGet`
等），彻底避免运行时按 `type_of` 做分支。

但是 Calcit 里最常见的集合高阶函数 `map` / `filter` / `reduce` 等都是在
`calcit-core.cirru` 里用 Calcit 本身定义的用户级 defn（例如 `&list:map`、
`&map:map`、`&set:filter`），原先不能在预处理阶段复用这套机制，运行时仍要靠
`list?`/`map?`/`set?` 三条 `cond` 分支来分发。

本次把能力扩展到 Calcit 级别的 core def，当 receiver 的静态类型可推断时，
直接把 `(map xs f)` 改写成 `(calcit.core/&list:map xs f)` 等形式。

## 关键改动

- `src/runner/preprocess/mod.rs`
  - `try_specialize_polymorphic_call` 新增 `file_ns: &str` 参数，用于构造
    `Calcit::Import` 头的 `ImportInfo::Core { at_ns }`。
  - 在已有的 proc 特化表之前新增一张“core def 特化表”：
    - `("map", List(_))   -> &list:map`
    - `("map", Map(_, _)) -> &map:map`
    - `("filter", List(_))   -> &list:filter`
    - `("filter", Map(_, _)) -> &map:filter`
    - `("filter", Set(_))    -> &set:filter`
  - 命中时直接构造 `Calcit::Import(CalcitImport { ns: "calcit.core", def, info:
    Core { at_ns: file_ns }, def_id: Some(program::ensure_def_id(...).0) })`
    作为新的 head，保持已 preprocess 过的参数不变，避免重复预处理。
  - 禁止对 `&list:map` / `&map:map` / `&set:map` / `&list:filter` /
    `&map:filter` / `&set:filter` 自身再次 monomorphize，避免潜在 cycle。
  - 调用点 `Ok(Calcit::from(CalcitList::from(ys)))` 前的 specialize 调用传入
    `file_ns`。

## 验证

- `cargo fmt && cargo clippy --release -- -D warnings` ✓
- `cargo test --release` ✓ 所有现有测试通过
- `yarn check-all` ✓ 全量集成测试 + WASM 测试全部 OK

## 后续思路

- 可继续把 `reduce` / `last` / `reverse` / `concat` / `to-list` / `to-set`
  等也加入特化表；其中 `concat` / `reverse` 已有 native proc（`NativeListConcat`
  / `NativeListReverse`），加到已有 proc 表即可。
- 可以考虑把谓词（`list?`、`map?`、`number?`、`string?` …）在类型已知时直接折叠成
  `true` / `false`，进一步帮 WASM / JS codegen 消冗余分支。
- 未来 `&set:map` 如果单独实现，即可补齐 `("map", Set(_))` 分支。

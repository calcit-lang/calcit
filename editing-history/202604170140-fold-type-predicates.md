# Fold type predicates when static type is known

## 背景

`list?` / `map?` / `set?` / `string?` / `number?` / `bool?` / `tag?` / `fn?` /
`tuple?` / `record?` 这些 type predicate defn 都是 `(&= (type-of x) :xxx)`
的形式。如果 preprocess 能拿到 `x` 的静态类型，整个调用就可以折叠成一个字面的
`true` 常量（或者保留运行时 check，但不把 `false` 折叠以保持安全）。

在 WASM / JS codegen 场景下这尤其有收益：谓词出现在 `if` / `cond` 里，
折叠成常量后整个分支会被后续 dead-code 消除。

## 改动

`src/runner/preprocess/mod.rs::try_specialize_polymorphic_call` 新增一段在
proc 特化表之前的折叠逻辑：

```rust
let predicate_true = matches!(
  (fn_def, receiver_type.as_ref()),
  ("list?",   T::List(_))
  | ("map?",    T::Map(_, _))
  | ("set?",    T::Set(_))
  | ("string?", T::String)
  | ("number?", T::Number)
  | ("bool?",   T::Bool)
  | ("tag?",    T::Tag)
  | ("fn?",     T::Fn(_) | T::DynFn)
  | ("tuple?",  T::Tuple(_) | T::DynTuple)
  | ("record?", T::Record(_) | T::Struct(_, _))
);
if predicate_true { return Some(Calcit::Bool(true)); }
```

**设计选择**：只折叠“肯定为 true”的情形，不折叠“肯定为 false”的情形。
原因：`CalcitTypeAnnotation` 枚举有 30+ variant（包括 `Optional`、`Ref`、
`TypeVar`、`TypeRef`、`Trait` 等等），要列全“除此之外的全部”枚举会很脆弱、
以后新增 variant 容易漏。保留 runtime check 是安全的 fallback。

## 验证

- `cargo fmt && cargo clippy --release -- -D warnings` ✓
- `cargo test --release` ✓（179 + 67 测试全通过）
- `yarn check-all` ✓

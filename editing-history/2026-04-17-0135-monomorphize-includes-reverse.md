# Monomorphize `includes?` and `reverse`

## 背景

继 `map` / `filter` 之后，继续扩展 `try_specialize_polymorphic_call`：

- `includes?` 原实现（calcit-core.cirru L3228）有 5 条 `if` 分支
  （`nil?` / `list?` / `map?` / `set?` / `string?` fallback `.includes?`）。
- `reverse` 实际上在 runtime 就是 `&list:reverse` 的别名，但以前每次调用还是要
  经过一层 Calcit user-def 间接。

## 改动

在 `src/runner/preprocess/mod.rs` 的 proc 特化表中追加：

```text
("includes?", T::List(_))   -> NativeListIncludes
("includes?", T::Map(_, _)) -> NativeMapIncludes
("includes?", T::Set(_))    -> NativeSetIncludes
("includes?", T::String)    -> NativeStrIncludes
("reverse",   T::List(_))   -> NativeListReverse
```

`concat` 因为是变长参数 defn，暂不在这里处理；后续可以在 preprocess 阶段
把已知都是 list 的 `(concat a b)` 展开为 `(&list:concat a b)` 的折叠形式。

## 验证

- `cargo fmt && cargo clippy --release -- -D warnings` ✓
- `cargo test --release` ✓
- `yarn check-all` ✓

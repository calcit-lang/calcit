# Specialize collection `update` contracts / 专门化集合 `update` 契约

## Context / 背景

The public compatibility schema for `update` must remain open because one signature cannot encode List, Map, Enum, and Struct member relationships. Return-type inference already preserved a statically known receiver, but argument checking only specialized Struct fields. As a result, known `List<T>` and `Map<K,V>` calls could still pass incompatible keys or updater functions.

`update` 的公共兼容 schema 需要同时覆盖 List、Map、Enum、Struct，单一签名无法表达全部成员关系。返回类型推断已能保留静态 receiver，但参数检查此前只专门化 Struct 字段，因此已知 `List<T>` / `Map<K,V>` 调用仍可能接受错误的键或 updater。

## Change / 修改

- Derive `Number` plus `T -> T` for `List<T>` update calls.
- Derive `K` plus `V -> V` for `Map<K,V>` update calls.
- Preserve the existing literal-field specialization for Struct values.
- Recover the statically inferable return type of fixed-arity inline callbacks whenever an expected `Fn` contract is available, rather than limiting that check to `option:fold`.
- Skip optional/rest callbacks because the current function annotation does not retain their call-shape semantics; this avoids treating valid shorthand lambdas as fixed arity.
- Add a type-fail fixture covering a bad List index, a named Map updater mismatch, and an inline updater with the wrong return type.

- `List<T>` 的 `update` 调用推导出 `Number` 索引与 `T -> T` updater。
- `Map<K,V>` 的 `update` 调用推导出 `K` 键与 `V -> V` updater。
- 保留已有的 Struct 字面字段专门化。
- 当调用点存在期望 `Fn` contract 时，恢复固定参数 inline callback 可静态推断的返回类型，不再只对 `option:fold` 生效。
- 对 optional/rest callback 保持原行为，因为当前函数 annotation 不保留其 call-shape 语义，避免把合法 shorthand lambda 错当成固定参数函数。
- 新增 type-fail fixture，覆盖错误 List 索引、具名 Map updater 类型不匹配，以及 inline updater 返回类型错误。

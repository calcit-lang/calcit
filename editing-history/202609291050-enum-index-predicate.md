# Enum 有效位置谓词

关联 #1482。`Enum .contains-index? Number -> Bool` 与 `contains-index? Enum Number -> Bool` 明确查询 tag（索引 0）及 payload（索引 1 起）的有效位置。只有非负、有限整数且小于 `&enum:count` 的索引返回 true；越界、小数和非有限值返回 false。具名 `ContainsIndex` trait 让严格方法调用能获得可证明的类型契约。

旧 `Contains .contains?` 的范围判断对 `0.5` 返回 true，而 `&enum:nth` 会拒绝该索引。为保留兼容契约，本次不改旧实现，也不提供无条件自动 fix。List/String 同名新方法仍保持各自既有契约。定义内 `:tests` 及 native、生成 JS、core WASM、WASI Component 共享验证；名义 `Option` 的方法查询应给出 proven 的 `Number -> Bool`。

# Specialize collection member contracts / 专门化集合成员契约

## Context / 背景

The public `get` and `includes?` facades support multiple collection families whose member relationships cannot be represented honestly by one current schema. Return inference and typed lowering already preserve much of `get`'s result evidence, but their all-Dynamic argument schemas caused checking to exit before a known receiver could constrain the key or member.

公共 `get` 与 `includes?` facade 同时支持多类集合，当前单一 schema 无法诚实表达所有成员关系。返回推断与 typed lowering 已保留大部分 `get` 结果证据，但全 Dynamic 参数 schema 会让检查器在已知 receiver 约束 key/member 之前提前退出。

## Change / 修改

- Generalize the existing core call-site specializer so an otherwise open core facade may still enter checking when its receiver supplies concrete evidence.
- For `get`, require Map key `K` or a `Number` index for List, String, and Enum receivers.
- For `includes?`, require Map value `V`, List/Set member `T`, or a String substring. Correct the existing internal `&map:includes?` schema and documentation, which incorrectly described value membership as key membership.
- Preserve the compatibility path when the receiver is Dynamic or unsupported, rather than inventing a closed global schema.
- Synchronize the Rust primitive metadata so `&map:contains?` binds `K` while `&map:includes?` binds `V`.
- Add unit coverage and a Snapshot type-fail fixture covering five mismatches.

- 泛化现有 core 调用点专门化，让全开放 facade 在 receiver 提供具体证据时仍能进入参数检查。
- `get` 对 Map 要求键 `K`，对 List、String、Enum 要求 `Number` 索引。
- `includes?` 对 Map 要求值 `V`，对 List/Set 要求成员 `T`，对 String 要求 substring；同时修正既有内部 `&map:includes?` schema 与文档中把 value membership 错写成 key membership 的问题。
- receiver 为 Dynamic 或无法识别时保留兼容路径，不虚构封闭的全局 schema。
- 同步 Rust primitive 元数据：`&map:contains?` 绑定 `K`，`&map:includes?` 绑定 `V`。
- 增加单元覆盖和包含五类错误的 Snapshot type-fail fixture。

# Resolve nominal Struct specialization / 解析 nominal Struct 专门化

Copilot's follow-up review noted that direct `TypeRef` annotations resolving to a Struct were excluded by the `assoc` specialization gate even though the shared field inference helper already supports them. The gate now accepts direct Struct annotations, Struct values, and resolvable direct TypeRefs. The same correction applies to `update`, which shared the narrower condition.

`Optional<Struct>` is deliberately not treated as a Struct receiver: Option is a nominal Enum wrapper at runtime and must be narrowed before `assoc` or `update` can target a Struct field. A regression assertion preserves that boundary so convenient type unwrapping does not create an unsound runtime dispatch assumption.

Copilot follow-up review 指出，能够解析为 Struct 的直接 `TypeRef` annotation 被 `assoc` specialization gate 排除，尽管共享字段推断 helper 已经支持它。gate 现在接受直接 Struct annotation、Struct value 以及可解析的直接 TypeRef；共享相同窄条件的 `update` 也同步修正。

`Optional<Struct>` 不会被当作 Struct receiver：Option 在运行时是 nominal Enum wrapper，必须先 narrow，`assoc` 或 `update` 才能针对 Struct 字段。新增回归断言固定该边界，避免便利性的类型解包制造不可靠的 runtime dispatch 假设。

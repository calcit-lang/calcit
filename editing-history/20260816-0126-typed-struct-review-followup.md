# Typed Struct review follow-up

- 匿名 Struct 文档同时列出 core/runtime 与显式可复用 `defimpl` 两类低层动态边界，避免文档规则比实际检查器更窄。
- 将 lexical-local 与 declaration-namespace 两套 TypeRef 解析改为共享 `map_type_refs_for_body` 遍历器；差异仅留在 TypeRef 回调，List/Map/Set/Ref/Optional/JsNullish/Variadic/Fn/Struct/Enum 的递归结构只维护一份。
- 保留原有解析顺序和 fallback：先递归处理类型参数，再执行局部或 namespace 解析；无法解析时仍保留原始 TypeRef 名称。
- 重点回归带引号局部类型和跨 namespace 嵌套 Struct 字段，防止维护性重构改变 issue #357 的行为。

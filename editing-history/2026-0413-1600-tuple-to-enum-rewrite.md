## 概要

为 tuple-to-enum 自动改写实现预处理阶段支持。当函数参数 schema 标注为 enum 类型时,
调用者传递的 untyped tuple `:: :tag payload` 自动改写为 `%:: Enum :tag payload`。

## 知识点

- `CalcitTypeAnnotation::resolve_to_enum_with_ref()` 对应 struct 端已有的 `resolve_to_struct_with_ref()`
- `resolve_enum_from_program()` 通过 `lookup_runtime_ready_registered` + `lookup_def_code_registered` 两级缓存查找
- tuple 改写只替换 head proc (`NativeTuple` → `NativeEnumTupleNew`) 并插入 enum 引用，不对 tag/payload 做验证（由 `check_enum_tuple_construction` 兜底）
- JS codegen 需要 `Calcit::Import` 而非 `Calcit::Enum` 内联值，通过 TypeRef 路径判断走 SameFile 还是 NsReferDef
- map-to-record 改写验证 key 合法性，tuple-to-enum 改写不验证（差异源于 struct field 是封闭集合，enum tag 验证已有独立检查）
- Cirru tag-match 零载荷变体的 pattern 是 `(:ok)` 而非 `:ok`（后者在 pair 中被当作 body）

# Nominal Trait and TypeRef matching

2026-09-02 20:25 CST

## English

- Match a resolved nominal `Trait` annotation with the corresponding qualified `TypeRef` retained by strict function schemas.
- Preserve qualified source identity when a strict generic `:where` bound resolves directly to a trait value, and allow a value represented by that same nominal trait annotation to satisfy the bound.
- Keep traits with identical short names in different namespaces distinct, and add regression coverage for both matching paths.
- Retain the documented compatibility behavior for an explicitly bare legacy trait placeholder while keeping qualified references nominal.
- This closes the false-positive `DomElement` argument and generic-bound warnings found while validating Respo against Calcit 0.13.74.

## 中文

- 让已解析的 nominal `Trait` annotation 与严格函数 schema 保留的对应限定名 `TypeRef` 正确匹配。
- 当严格泛型 `:where` bound 直接解析为 trait value 时保留限定 source identity，并允许由同一 nominal trait annotation 表示的 value 满足该 bound。
- 对不同 namespace 中短名称相同的 trait 继续保持 nominal 区分，并为两条匹配路径增加回归测试。
- 对显式 bare legacy trait placeholder 保留已有兼容匹配行为，同时继续保证限定名引用的 nominal 区分。
- 修复 Respo 使用 Calcit 0.13.74 验证时出现的 `DomElement` 参数与泛型约束误报。

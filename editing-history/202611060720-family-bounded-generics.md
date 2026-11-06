# 族约束泛型 `:where (T AnyEnum)`（#2611）

方向见 Jon 2026-11-06：逐步减少生态里的 `AnyEnum` / `AnyStruct`。只做反射的函数改用族约束的类型变量，调用方的具体类型可以穿过函数。

- 解析：`:where (T AnyEnum)` / `(T AnyStruct)` 仍解析成同名的 `CalcitTrait` 引用。`family_of_bound_trait` 通过“名字是 AnyEnum/AnyStruct、没有 runtime id、没有定义引用、没有方法”识别它们。
- 调用点：`proves_trait_bound` 先走 `family_bound_proof`。实参必须是该族的具名成员（借 `open_family_type` 与 `is_proven_for` 判定）；`Dynamic`、匿名 enum、Custom、族类型本身，以及没有同一族能力的类型变量都不满足。
- 函数体：`resolve_where_bound_type_for_body` 保留族 trait，带该能力的 TypeVar 能证明到族类型（按形状匹配，替换后指针身份不再可靠）。返回类型仍是调用方的具体类型。
- 报告：`W_GENERIC_WHERE_BOUND_MISMATCH` 对族约束改为 "family bound `AnyEnum`, which needs a named enum; decode or narrow the value to a concrete enum first"（AnyStruct 同理）。

验证：新 fixture `calcit/type-fail/family-bound-generics.cirru`，默认 entry 在 `--strict-types` 下通过，五个 reject entry 都给出族约束的告警；CLI 测试 `strict_family_bounds_admit_named_members_only`。

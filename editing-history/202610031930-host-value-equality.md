# 宿主句柄的值比较与哈希检查

关联 #1704 缺口 2(b)。运行时修复 #1706 已让宿主对象按引用身份比较与哈希；本次在静态检查中拒绝把已证明的 external-object trait 值用于值比较或哈希位置，提示改用 `identical?` 或先解码。

判定只认声明为 `:kind :external-object` 的 trait（含 Optional / JsNullish 包装），复用 `trait_is_external_object`，不建立宿主种类表。裸 `JsObject` 保持开放：在 Respo main 上试跑时，`aget` 读取 dataset 得到的 `JsNullish<JsObject>` 实际是字符串，按 JsObject 报告会产生 16 条误报。`Dynamic` 与未解析参数也不报告。

检查位置：core `=` / `not=` 的全部参数，proc `&=` 的全部参数，`#{}`、`&include`、`&exclude`、`&set:includes?` 的成员参数，`&{}`、`&map:assoc`、`&map:get`、`&map:dissoc`、`&map:contains?` 的键参数。没有把它写成 `=` 的 `Eq` where 约束：where 约束要求 Dynamic 先收窄，会改变现有开放比较（core `=` 的 `compares-open-binding-symmetrically` 测试）。因此新增诊断编号 `W_HOST_VALUE_EQUALITY`，它只反驳已证明的宿主句柄，不要求其他值提供 Eq 证据。

验证：`calcit/type-fail/host-value-equality-strict.cirru` 由 Rust 测试检查四条诊断及 JsObject / `identical?` 不报告；js-ffi 模块 fixture 的 `checked-counter-host` 新增 `explicit-identity` :tests，由 `scripts/check-js-ffi-source.mjs` 回放生成 JS，并确认对宿主句柄使用 `=` 的构建失败。js-ffi main 与 Respo main 的 check-public 均无新增诊断。缺口 3（外部 trait 子类型）不在本次范围。

# trait 的 'requires 子句

关联 #1535（requires 部分）与 #1704 缺口 3，设计见 `RFCs/10-03-trait-requires-rfc.md`（表层写法已由维护者审阅：Symbol 子句头 `'requires`、同名成员一律报错、声明期错误合并为 `E_TRAIT_REQUIRES`）。

内部的 `CalcitTrait.requires`、传递规范化、环检测、按 origin 证明与 `js-cast` 可达成员检查此前已存在，本次只补声明入口与缺失的检查：`deftrait` 宏把 `('requires P)` 子句拆出为 `&trait::new` 的可选第三参数（native、JS 运行时都接受），源码解析同样读取子句并按命名空间解析父 trait（递归带活动栈，环交给声明检查报告）。

声明检查在预处理 `&trait::new` 调用时运行，子 trait 从源码解析（external-object 的字段成员在 native 不能由运行时构造器求值）。普通 trait 的 `impl-traits` 在 native/JS 运行时和预处理（core `impl-traits` 调用）都要求父 trait 实现已挂载，否则 `E_IMPL_MISSING_REQUIRED_TRAIT`；只有这样，`impl_matches_trait` 沿可达集合把子 trait 实现当作父 trait 证据才是可靠的。`assert-traits` 同样检查父 trait。

external-object 侧：trait 类型之间的兼容关系与 TypeRef 匹配沿可达集合成立（隐式上转），字段查找遍历 `requires`，JS 宿主名映射在可达 trait 中查找成员所属 trait 的 `:names`。`defexternal` 简写接受 `'requires` 条目。

验证：`calcit/test-traits.cirru` 的 `test-trait-requires` 在 native 与 JS 运行；五个 type-fail fixture 由 Rust 测试检查两类诊断；js-ffi 模块 fixture 的 `inherited-members` :tests 回放到生成 JS（上转、继承的映射字段与方法）。未做：`query type` 列出继承成员、js-ffi 事件 trait 的下游迁移。

# nil->option 参数改为泛型 T 并登记 Optional 删除版本

## 问题与方向

`nil->option` 是 core 里最后一个仍以 `(Optional T)` 作为公共参数类型的定义，靠 `legacy_optional_public_schema_requires_migration` 中按名字的豁免避开 `E_LEGACY_OPTIONAL_SCHEMA`。这既让 Optional 无法被删除，也让一个公共 API 写着它自己拒绝的旧类型。关联 #1554。

## 方案

参数改为泛型 `T`，返回仍为 `Option<T>`；删除按名字的豁免，只保留 core `&` 原语的豁免。调用点不变：`nil` 得到 `Option :none`，其余值（含已有 `Option`）得到 `Option :some value`。

旧参数类型有一个未写明的作用：`JsNullish<T>` 传入 `Optional<T>` 形参需要边界证明，所以 docs/features/js-interop.md 与 polymorphism.md 才能说“泛型 `nil->option` 不接受 JsNullish”。换成普通 `T` 后这条拒绝消失（`T` 直接绑定为 `JsNullish<Number>`，返回 `Option<JsNullish<Number>>`）。没有放宽通用类型关系，也没有给所有泛型槽加宿主可空限制，而是沿用 `check_user_fn_arg_types` 中已有的“按 core 函数名附加迁移提示”的写法，仅对 core 的 `nil->option` 在参数检查循环里把 `JsNullish` 实参判为不匹配，并给出 `js-present?` / `js-nullish->option` 的迁移提示。这是临时的按名字规则，Optional 变体删除并让 `JsNullish` 的边界原因独立后，应改成通用的“泛型槽拒绝宿主可空值”或 where 约束（#1554 后续）。

## 兼容边界

- 调用方不变；`nil->option` 的参数变宽，返回类型相同。
- `docs/data/core-api-0.28-baseline.edn` 中该定义的 `:args` 随之改变。冻结基线检查（`scripts/core-api-contract.mjs`）按设计拒绝任何已冻结签名变化，包括与 HEAD 或基线分支的历史比较。该检查新增一条窄规则：把参数位置的 `(:: 'Optional X)` 视为与 `X` 等价（输入变宽，调用方兼容）；返回位置、具体类型替换仍然报错，并在 `core-api-contract.test.mjs` 中加了正反例。
- 诊断 `E_LEGACY_OPTIONAL_SCHEMA` / `W_LEGACY_OPTIONAL_SCHEMA` 的文字加入删除版本 0.32.0，诊断编号不变；不新增 fix 规则，迁移交给 `W_`/`E_` 定位加 `edit` 事务（见 docs/run/upgrade.md）。
- `docs/run/upgrade.md` 登记 0.32.0 删除 `'Optional` 公共 schema 与旧 Optional 类型，并改写已过期的“105 处/19 处”盘点；`docs/run/strict-nil-migration.md` 同步。

## 验证

- `nil->option` 的 definition `:tests` 增加普通类型值（String、List）的 `assert-type` 与结果断言，复用原有 nil/falsey/嵌套 Option 用例。
- `JsNullish` 实参仍被拒绝：用 `src/bin/cr_tests/type_fail.rs` 的 Rust 测试，因为 `:tests` 是在检查通过之后运行，不能表达预期的预处理诊断。同一测试还确认普通调用不产生警告。
- `src/snapshot.rs` 的 schema 单测改为断言裸泛型 `T` 与 `Option<T>` 输出共享同一类型变量；`mod.rs` 中的豁免单测改为断言 `nil->option` 不再被豁免。

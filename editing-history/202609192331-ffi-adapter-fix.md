# JS FFI 迁移建议形态评估与固化（#1224 阶段二）

- 评估结论：从裸 `JsObject` 访问到 external-object trait 的迁移**不能做 machine-applicable 自动改写**。
  trait 名字、payload 类型与宿主名映射都无法从成员访问唯一推断；自动生成会产出错误契约。
- 因此 review surface 采用既有的 `calcit fix --workflow strict` 报告：`review_required.ffi_boundaries`
  已按 definition 分组给出 `operations`（kind/member/path、稳定 `I_FFI_BOUNDARY_EVIDENCE` code）与
  trait candidate；阶段一加入的 `defexternal` 骨架也随之出现在该报告中。
- 新增集成测试 `tests/fix_cli.rs::strict_workflow_surfaces_ffi_boundary_defexternal_skeletons`：
  构造含 `(.-length element)` 的 `:js-ffi` 定义，断言 strict workflow 报告中出现
  `defexternal RawReadHost (:length 'Dynamic)` 与 `contract_status = review-required`。
- `docs/run/fix.md` 新增「JS FFI 边界迁移（review-only）」：说明盘点 → 骨架 → `calcit edit def` 补全
  类型 → 收敛访问 → 验证的流程，并解释为何不进入可 apply preset。
- 验证：`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test` 全绿；
  docs check-md 69/69。

## Review 跟进（PR #1231）

- tiye 指出 `docs/run/fix.md` 里骨架示例的括号多余，并建议用 `calcit cirru parse` 验证结构。
  验证发现这是真实 bug：多行形式下 `(:length 'Dynamic)` 会被解析成多一层嵌套（`[[":length", ...]]`），
  `defexternal` 解析器会拒绝；只有单行写法才是等价的。
- 修复：`defexternal_skeleton` 改为无括号的多行成员形式
  （`:length 'Dynamic`、`.query $ :: 'Fn $ {} ...`），并同步 `docs/run/fix.md`
  与 `docs/features/js-interop.md` 示例。
- 单测 `defexternal_skeleton_filters_unexpressible_members` 增加「生成骨架必须通过
  `validate_defexternal_shorthand`」断言，防止再次回归；集成测试期望串同步更新。

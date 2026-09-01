# Reject Nil for Unit in strict mode / strict 模式拒绝用 Nil 冒充 Unit

## Summary / 概要

- Added `E_NIL_FOR_UNIT` for functions whose declared return is `Unit` but whose
  statically resolved returned expression is `Nil`.
- Kept ordinary mode on the existing `W_FN_RETURN_TYPE_MISMATCH` migration path.
- Reused macro call-site recovery so inline `fn` diagnostics point to user
  source rather than the core macro expansion.
- Documented `&unit` and Unit-returning effects as the deterministic migration.

## Boundary / 边界

The check only examines the function return expression after preprocessing. An
intermediate nil remains an ordinary Nil value and is not relabeled as a Unit
contract violation. The hard diagnostic is restricted to project namespaces in
`--strict-types`; dependencies and compatibility-mode execution retain their
existing behavior.

检查只作用于预处理后的函数返回表达式。中间位置的 nil 仍是普通 Nil，不会被误判为
Unit 返回违约。硬错误只在 `--strict-types` 的项目命名空间生效；依赖和兼容模式保留
原有行为。

## Verification / 验证

- Unit test covers compatibility and strict branches with the stable error code.
- Manual eval verifies both returned `nil` and legacy `;nil`, plus successful
  `&unit`, including the user macro call-site location.

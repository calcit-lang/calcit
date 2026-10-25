# 泛型 struct 字面量的空容器字段保持返回 binder

## 问题与依据

#2279（c819ea5）之后，普通 `--check-only` 拒绝正确代码：返回类型声明为 `EmptyFieldBox 'T 'S` 的函数，用 `([])` 填 `:items` 字段时，构造器推断把 `T` 变成 Dynamic，返回证明随后报 `E_ERASED_GENERIC_RELATION`。生态中 `reel.typed/new-reel`（State 的 `:records ([])`）因此失败，连带 calcium-workflow、cumulo-reel、hn-reader 等 Respo 系项目；0.31.0-alpha.2 通过。

## 修复边界

返回证明遇到 `%{}` 字面量且期望是带实参的泛型 struct 时，按声明的实参逐字段证明，而不是先从字段值反推构造器类型。字段类型由声明的 struct 和期望实参得到；空容器字段因此保留调用方 binder。字段值与声明实参真正不符（如 `[] 1`、`{}`、与 `'S` 不一致的 seed）仍按原诊断拒绝。非字面量、非泛型期望继续走原路径；不新增诊断、规则或 Dynamic。

## 验证

`test-types-inference` 增加 `keeps-element-binder-of-empty-field`（native/js/wasm 通过，原二进制下失败）。`check-known-assertion.mjs` 覆盖接受与三种拒绝，在普通检查、concrete-return 规则和 strict 下分别断言。loop 累加器 `[]` 的同类误报不在本次范围内。

# `non-nil?` 谓词迁移

## 背景

旧的自由函数 `some?` 实际只判断值不为 `nil`，但名字容易与名义 `Option` 的 `.some?` variant 判断混淆。WASM 对 false、0 与 nil 的表示差异已经先通过类型证据修复，因此本阶段可以在不固化 backend 缺陷的前提下收敛表层名称。

## 决策

- 新增 `non-nil?: T -> Bool` 作为普通值判空的首选入口；false、0、空集合和 `Option :none` 都是非 nil。
- 旧 `some?` 在兼容窗口内保留原语义，不原地改成 Option 判断。Option 使用 `.some?/.none?` 或 `match`。
- `JsNullish<T>` 是宿主边界，`non-nil?` 与旧谓词一样不能绕过 `js-present?` / `js-nullish?`。
- 自动迁移继续收拢在 `calcit fix`。`core-non-nil-predicate-v1` 只改写 compiler-resolved `calcit.core/some?`，用限定名避免新名称被遮蔽；未知 macro 保留人工 review。
- 不修改已经发布的 preset。等 #1454 的其余 predicate/member 规则稳定后，再把本批规则纳入新的版本化 preset。

## 验证

- Calcit definition `:tests` 直接覆盖 nil、false、0、空集合、Option 两个 variant、类型化参数与泛型转发。
- 同一测试 AST 在 native、生成 JavaScript、core WASM 与 WASI 0.3 Component 重放。
- Rust 只验证 fix 的 resolver/source transaction 边界，以及 Option/JsNullish 的预处理诊断边界。
- fix 验证 preview、apply、revision guard、重复预览幂等与应用后的 definition test。

# 修复跨 namespace 宏生成闭包的 FFI 能力继承（#1232 编译器缺口）

## 背景

`calcit fix --workflow strict` 在真实项目 respo.calcit 上无法产出计划：
`--check-only` 通过，但迁移规划阶段报
`external-object method call used in respo.app.comp.todolist/?? without :js-ffi feature in schema`。

## 根因

- `calcit fix` 的迁移规划会调用 `compile_selected_definitions_for_migration`，临时关闭 strict types，
  以便在旧源码上收集 warning 与类型证据（见 `docs/run/fix.md`）。
- 但 `js-ffi` 能力检查（`require_js_ffi_feature`）并不依赖 strict types，只看 entry 的
  `:feature-policy`。在 strict 诊断默认下，该策略是 `:error`。
- 同时，宏生成函数的能力继承被写成 `strict_generated_by_macro = strict_types_enabled() && generated_by_macro`
  （`src/runner/preprocess/mod.rs`）。规划阶段 strict 关闭后，宏展开出的匿名 `fn` 不再继承父级
  `:features`，于是能力检查在 `CURRENT_FN_FEATURES = None` 时失败。
- 触发条件需要**跨 namespace 宏**：宏从 `respo.core` `:refer` 到 `respo.app.comp.todolist` 后展开，
  生成的匿名 `fn` 的 `def_name` 退化为 `??`，`program::lookup_def_schema` 查不到，无法回退到
  定义 schema 的 features；只能依赖词法父级继承。同 namespace 宏会经 `def_name` 命中 schema，掩盖了问题。

## 修复

`src/runner/preprocess/mod.rs`：把能力继承从 `strict_generated_by_macro` 改为 `generated_by_macro`。
能力边界是词法属性，与是否开启严格类型无关；返回值推断与 `generated_fn_schema` 合成仍保留 strict gating。

## 回归测试

- 新增 `tests/fixtures/fix-cross-ns-macro-ffi.cirru`：两个 namespace，`defprobe.macros/defeffect`
  生成 `defn`，其 `:method` 匿名 `fn` 对 `unsafe-coerce` 后的 external-object trait 调用 `.select!`；
  entry 声明 `:feature-policy (:js-ffi :error)`。
- 新增 `tests/fix_cli.rs::strict_workflow_plans_cross_namespace_macro_generated_ffi`：
  断言 `--check-only` 通过，且 `fix --workflow strict` 能产出 `status = planned`。
  该 fixture 在修复前会稳定地让规划报 `E_JS_FFI_FEATURE_REQUIRED`。

## 验证

- `cargo fmt`、`cargo clippy -- -D warnings`、`cargo test` 全绿。
- respo.calcit 上 `fix --workflow strict` 不再报 FFI planning 错误（后续阻断为独立的非 FFI 严格债）。

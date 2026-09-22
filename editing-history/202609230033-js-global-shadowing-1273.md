# 避免生成的 JS import 遮蔽宿主全局对象（#1273）

## 背景

Calcit 的 `js/Element` 生成裸 `Element`。同一模块若 `:refer` 导入同名的 Calcit schema `Element`，
生成的 `import { Element }` 会在模块词法作用域内遮蔽 `globalThis.Element`，导致 Respo 的 Node DOM smoke
在 `instanceof` 处报 `TypeError: Right-hand side of instanceof is not callable`。`js/globalThis.Element`
只是绕过，不是期望的编译器行为。

## 根因

`src/codegen/emit_js.rs` 的 `Calcit::RawCode(RawCodeType::Js, code)` 直接输出原始代码，不区分该名字是否
已被同模块的 def 或 import 绑定。

## 修复

- 新增 `qualify_js_host_global(ns, code)`：取 `code` 的根标识符，若它是当前 namespace 的模块级绑定
  （`file.defs` 或 `file.import_map` 的转义名），则把引用限定为 `globalThis.<code>`。
- 已显式写 `js/globalThis.X`、非标识符或没有同名绑定时保持原样，避免无意义输出变化。
- 该逻辑同时覆盖 `exists?`（它复用 `to_js_code` 输出 `typeof <target> !== 'undefined'`）。

## 测试

- Rust 单测 `codegen::emit_js::tests::js_host_global_reference_qualifies_only_on_module_binding_collision`：
  覆盖 import/def 冲突、`globalThis` 前缀、非冲突全局与 `js/typeof` 运算符。
- 集成测试 `tests/js_global_shadow_cli.rs` + `tests/fixtures/js-global-shadow.cirru`：
  模块导入 Calcit `Element` 且使用 `js/Element`，断言生成 `import { Element }` 与 `globalThis.Element`，
  且 `exists?`/值读取都走宿主全局。该测试在未修复时稳定失败。

## 验证

- `cargo fmt`、`cargo clippy -- -D warnings`、`cargo test --release` 全绿。
- `yarn try-js`、`yarn try-ir`、`yarn check-js-runtime`、`yarn check-strict-default`、`yarn try-wasm` 通过。
- 真实消费者：Respo `main`（0.18.1）用本修复构建后 `yarn test-dom-host` 通过；Respo 当前仍保留
  `js/globalThis.Element` 写法，因此不阻塞，但编译器已不再依赖该 workaround。

# Nominal Option API migration and JS FFI diagnostics

## 修改概要

- 直接将 `find`、`find-index`、`index-of` 的公开返回值迁移为名义 `Option`，不再保留并行的 safe API；
- 将公开 `parse-float` 改为 `Result<Number,String>`，将公开 `get-env` 改为单参数 `Option<String>`，nullable 宿主过程下沉为 `&parse-float` 与 `&get-env`；
- 对未绑定泛型只把 payload 降为 Dynamic，保留 `Option<Dynamic>` 等外层名义类型，避免迁移信息整体退化为 Dynamic；
- 新增 `W_NOMINAL_ENUM_LEGACY_USE`，发现 Option/Result 继续被 `some?`、`nil?`、位置读取或底层比较按旧 nullable 值消费；
- 将 JS 属性读取、原生方法调用、`aget`/`js-get` 与 `js/...` 调用推断为 `Optional<JsObject>`，并新增 `W_JS_FFI_NULLABLE_DEREF`，不把宿主空值自动转换成 Option；
- 统一 Native、JS、WASM 中空 List 的 `rest`/`butlast` 行为，并收紧 `rest`、`empty` 的同类型返回契约；
- 更新 core examples、Native/JS/WASM 测试、迁移 RFC 与 JS interop/Option 文档。

## 知识点

- breaking API 只有在外层名义类型不被 Dynamic 吞掉时才具有可迁移性；未知 payload 应表示为 `Option<Dynamic>`，而不是把整个调用结果退回 Dynamic；
- `some?`/`nil?` 只适合 `Optional<T>` 兼容边界。名义 Option 必须使用 `option:some?`、`option:none?`、unwrap 或 `tag-match`，否则 `%none` 作为非 nil tuple 会造成分支静默反转；
- JS FFI 的两个风险维度必须同时保留：`Optional` 表示 `null`/`undefined`，`JsObject` 表示 payload 仍是未验证的宿主对象。nil guard 不能把 JsObject 自动证明成 Number/String；
- `first`、`last`、`nth`、`get` 参与 core 宏自举，不能在宏迁移到明确低层原语前直接改成 Option；否则 `let` 等宏会在加载阶段收到名义 tuple；
- 完整 core Snapshot 的递归预处理和 context 渲染需要与 CLI 相同的 16 MiB 栈预算，Rust 默认测试线程栈不足以覆盖增长后的 core AST。

## 验证

- `cargo fmt --all`
- `cargo clippy -- -D warnings`
- `cargo test`（346 lib + 2 caps + 180 cr）
- `yarn check-all`（Native、JS、IR、WASM 全通过）
- `yarn check-agent-interface`（12/12，通过 `check-all` 执行）
- `cr docs check-md docs/features/polymorphism.md --entry calcit/test.cirru --failures-only`
- `cr docs check-md docs/features/js-interop.md --entry calcit/test.cirru --failures-only`
- 安装当前 `cr` 后只读检查 Respo：报告 3 条 Option 迁移、1 条 `get-env` arity、10 条 JS FFI nullable dereference、6 条 JS/Unit 返回不匹配；Respo 工作区保持干净。

# JS FFI 方法调用体验评估与文档引导（#1217）

- 在 `docs/features/js-interop.md` 新增「3.3 方法调用体验」：external-object trait 声明示例、
  边界 coercion、`(receiver .method args...)` 调用、链式返回类型、构造器/静态方法、`JsNullish` 收窄、
  `:features :js-ffi`，以及与 JavaScript 的对照表。
- 明确当前边界：Calcit 有意不引入 `Promise<T>`/`Iterator<T>` 等泛型宿主 trait；适配器应在边界消费
  这类泛型宿主值，只向业务返回 `Option`/`Result`/`Struct`/`Enum`。
- 评估结论与 follow-up：
  - 支持：trait 字段/方法静态派发、`:names` 映射、`:writable`、`js-get`/`js-set`、`JsNullish` 收窄、
    `E_UNTYPED_JS_OBJECT_ACCESS` 等诊断、`--ffi-evidence`/`--warn-dyn-method` 盘点。
  - 待改进：声明样板较重 → #1223（`defexternal` 简写）；裸 `JsObject` 迁移无自动 fix → #1224。
  - 有意不支持：泛型宿主 trait（按 RFC 08-18），用适配器模式替代。
- 验证：`CI=true CALCIT_DOCS_CHECK_JOBS=4 bash scripts/check-docs-md.sh` 69/69（新增 `.no-check`
  示例均通过解析校验）。

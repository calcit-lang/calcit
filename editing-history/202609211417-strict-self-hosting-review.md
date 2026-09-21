# 处理严格自举 PR 的 review 与文档回归 / Strict self-hosting review follow-up

## 中文

- 修正 WASM rest-list 与 `test-println` 的 schema、`&map:vals` 的 List 返回契约，并删除已经等价的余数 benchmark 变体。
- 扩大严格默认入口防回退检查范围，同时只允许 WASI 旧快照的明确兼容回归保留开关。
- 修复 11 个文档页面上的严格类型代码块；推荐写法改用具名函数或明确类型提示，展示历史兼容与预期错误的片段明确不运行。
- 区分真实源码定义与宏生成回调，避免真实定义从宏调用链继承 FFI 能力；函数体 `hint-fn` 不能抹掉公开 schema 声明的 `:js-ffi` 能力。`if-let`、`slice` 和 `range-bothway` 三个既有 core API 保留现有可选参数约定。

## English

- Correct WASM rest-list and `test-println` schemas and the list-valued `&map:vals` contract; remove redundant remainder benchmark variants.
- Extend the strict-default entrypoint guard while retaining one explicit compatibility regression for the captured legacy WASI Snapshot.
- Update strict-mode documentation examples across 11 pages, marking intentional legacy or error demonstrations as non-executable.
- Distinguish source-owned definitions from generated callbacks under macro ancestry so source definitions do not inherit ambient FFI capability. An embedded `hint-fn` must not erase `:js-ffi` from the public schema. Preserve the existing optional-parameter conventions of three core APIs.

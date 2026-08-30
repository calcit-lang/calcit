# 版本化 FFI Interface IR 导出 / Versioned FFI Interface IR export

## 中文

- 为 `calcit ffi export` 增加只读、确定性的 JSON/人类可读 inventory，作为 typed FFI bindgen 的稳定输入。
- v1 分离 backend-neutral `:schema` 与 `:ffi` lowering，只选择带 lowering 字段的本地 raw binding；普通定义中的空 `:ffi {}` 或仅 `:features` capability 元数据不会污染接口，畸形元数据仍会进入诊断。
- 不可表示的 Dynamic、callback、Map/Set、Ref、host object、泛型或可变参数边界产生结构化 diagnostic，禁止静默动态降级。
- 提供打包的 JSON Schema、可检索双语文档和 agent stdout 守门；真实 `regex` 模块回归识别到 4 个 native binding，其中 3 个现有 Dynamic 边界被明确标记 unsupported。
- 验证通过：`cargo clippy --all-targets -- -D warnings`、`cargo test`、`yarn check-all`。

## English

- Add a read-only, deterministic JSON and human inventory through `calcit ffi export` as the stable input for typed FFI bindgen.
- Keep backend-neutral `:schema` separate from `:ffi` lowering and select local raw bindings with lowering fields; empty `:ffi {}` or capability-only `:features` metadata does not pollute the interface, while malformed metadata remains diagnostic-visible.
- Emit structured diagnostics for Dynamic, callbacks, Map/Set, Ref, host objects, generic callables, and variadic boundaries instead of silently degrading to dynamic generation.
- Bundle the v1 JSON Schema, indexed bilingual guidance, and an agent stdout contract check. A real `regex` regression identifies four native bindings and explicitly marks its three remaining Dynamic boundaries unsupported.
- Verified with `cargo clippy --all-targets -- -D warnings`, `cargo test`, and `yarn check-all`.

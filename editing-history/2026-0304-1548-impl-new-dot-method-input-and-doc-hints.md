# 2026-03-04 15:48 `&impl::new` dot-method 输入与文档提示对齐

## 背景

在 trait/impl method key 迁移到 `.method` 的过程中，声明层（`deftrait`/`defimpl`）已支持并迁移，但原生构造入口 `&impl::new` 在不同后端对 `.method` 输入支持不一致：Rust 侧与 JS 侧行为存在差异，导致 `yarn check-all` 的 JS 路径报错。

## 本次改动要点

- 扩展 `&impl::new` 字段键解析，支持 `.method` 输入并规范化为内部 tag 键：
  - Rust: `src/builtins/records.rs` 接受 `Calcit::Method` 作为 field key。
  - JS: `ts-src/calcit.procs.mts` 为 method closure 写入 `__calcitMethodName` 元信息；`ts-src/calcit-data.mts` 的 `castTag` 可读取该元信息并转为 tag。
- 增加回归用例：
  - `calcit/test-doc-smoke.cirru` 新增 `test-native-impl-new-dot-method`，直接覆盖 `&impl::new` + `.method` 场景。
- 补齐文档与示例提示（用于 `query examples` 与错误 hint 链路）：
  - `src/cirru/calcit-core.cirru` 新增 `calcit.core/&impl::new` 的 `CodeEntry` 和 `.method` 示例。
  - 更新 `&impl:get` 参数说明支持 `.method`，并新增 `&impl:get DemoImpl .show` 示例。

## 迁移语义结论

- 声明层：推荐 `.method`，legacy `:method` 兼容（并有 warning 提示）。
- 存储层：继续保持 tag 作为内部键（兼容和最小改动优先）；本次仅扩展输入层与提示层，不改内部数据结构。

## 验证

- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `yarn compile`
- `cargo test`
- `yarn check-all`
- `cargo run --bin cr -- demos/compact.cirru query examples 'calcit.core/&impl::new'`
- `cargo run --bin cr -- demos/compact.cirru query examples 'calcit.core/&impl:get'`

以上在本次改动后均通过。

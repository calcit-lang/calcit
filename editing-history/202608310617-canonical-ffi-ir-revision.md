# 规范化 FFI IR revision 输入 / Canonicalize FFI IR revision input

## 中文

- 根据 PR #537 review，修复 unordered EDN map/set 经普通 formatter 输出顺序不稳定、从而导致等价接口 revision 变化的问题。
- 对 `logical_schema` 与 `lowering.raw` 使用递归 canonical 表示：map/struct 按键排序，set 按规范化值排序，list/enum 保留语义顺序。
- 增加相同 metadata 以不同插入顺序构造时 raw 输出与 revision 完全一致的回归测试。
- 验证通过：`cargo test ffi_interface_ir`、strict clippy、agent interface 18/18，以及真实 `calcit-lang/regex` inventory。

## English

- Address PR #537 review by preventing unordered EDN map/set formatter iteration from changing revisions for equivalent interfaces.
- Use a recursive canonical representation for both `logical_schema` and `lowering.raw`: sort map/struct keys and set values while preserving list/enum semantic order.
- Add a regression proving that metadata assembled in different insertion orders produces identical raw output and revision.
- Verified with `cargo test ffi_interface_ir`, strict clippy, agent interface 18/18, and the real `calcit-lang/regex` inventory.

# Directional Component Interface export

## 中文

- 在现有 `calcit ffi export` 下增加 `--boundary component`，不增加新的顶层命令。
- 从 typed `defwasm-import` / `defwasm-export` 导出带 direction、module 和 symbol 的 Component Interface IR v1。
- 复用 Interface IR v2 的 schema conversion 与 reachable declaration 逻辑，在 boundary 拒绝 generic nominal、optional/rest arity 和重复 binding。
- Component contract 默认使用 Cirru EDN，支持 `--format human|edn|json`；现有 native human 与 `--json` 行为保持兼容。
- 增加版本化 JSON Schema、稳定 revision 和针对 direction、reachable type、generic、arity 与 symbol conflict 的测试。

## English

- Added `--boundary component` under the existing `calcit ffi export` entry point without introducing a top-level command.
- Exported Component Interface IR v1 from typed `defwasm-import` / `defwasm-export` definitions with direction, module, and symbol identities.
- Reused Interface IR v2 schema conversion and reachable declarations while rejecting generic nominals, optional/rest arity, and duplicate bindings at the boundary.
- Defaulted Component contracts to Cirru EDN with explicit `--format human|edn|json`, while preserving existing native human and `--json` behavior.
- Added a versioned JSON Schema, stable revisions, and tests for direction, reachable types, generics, arity, and symbol conflicts.

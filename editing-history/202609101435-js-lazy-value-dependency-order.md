# JS LazyValue dependency ordering / JS LazyValue 依赖排序

## Problem / 问题

The JavaScript backend could initialize a top-level `impl-traits` alias before the local `defimpl` value it referenced. `CompiledDef::deps` contained the complete relation, but the previous insertion-based sorter could still place an early lexical dependent ahead of dependencies discovered later.

JavaScript 后端可能在其引用的本地 `defimpl` 值之前初始化顶层 `impl-traits` 别名。虽然 `CompiledDef::deps` 已包含直接依赖，旧的单遍插入排序仍可能把字典序靠前的依赖方放到后续出现的依赖之前。

## Change / 修改

- Replace the depth-limited insertion heuristic with a deterministic dependency-first DFS ordering.
- Add a regression graph matching the constructor → attached type → enum/impl initialization chain.

- 用确定性的依赖优先 DFS 排序替换深度受限的单遍插入启发式。
- 添加覆盖“构造器 → 附加 trait 的类型 → enum/impl”初始化链的回归测试。

## Verification / 验证

- `cargo test codegen::emit_js::deps::tests`
- `cargo test`
- `cargo clippy -- -D warnings`
- `yarn compile`
- `yarn check-agent-interface`
- `yarn check-all`
- Built the patched `calcit` binary and used it to generate/run both JavaScript and strict type checks for `calcit-lang/calcit.algebra`.

- 使用修复后的 `calcit` 二进制生成并运行 `calcit-lang/calcit.algebra` 的 JavaScript 后端，同时验证严格类型检查。

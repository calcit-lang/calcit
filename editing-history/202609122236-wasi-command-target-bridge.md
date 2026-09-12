# WASI command target bridge

## 中文

- `cr-wasm` 现在显式接受 `--target core|wasi`，默认 `core` 保持现有浏览器与嵌入式宿主 ABI。
- `wasi` 目标增加标准 `_start: () -> ()` command 入口，入口调用选中的零参数 Calcit `init-fn`，忽略 Calcit 返回值。
- host imports 由 target 选择并集中登记。首个 WASI bridge 不继承 core 的 JS `math`/`io` imports，也拒绝任意 `defwasm-import`，缺失能力以 `E_WASM_CAPABILITY` fail closed。
- import 索引保留 registry 中最先登记的 capability，用户声明不能用重复 module/field 静默覆盖编译器内建 lowering。
- Preview 1 只作为内部 core-module 兼容层；公开 Calcit API 后续按 args/env/stdio/exit 等 capability 建模，避免泄漏具体 ABI 名称。
- 用户可观察的 command 语义放在 `calcit/test-wasi-command.cirru` 的 definition `:tests`；shell 只负责验证 target 错误、capability 错误与 Wasmtime 启动。
- `--check-only` 与 emit 共享相同的 WASI target validation，但不会写出 module，避免检查通过后真正生成才因 entry/capability 失败。

## English

- `cr-wasm` now accepts an explicit `--target core|wasi`; `core` remains the default and preserves the existing browser/embedded host ABI.
- The `wasi` target adds a standard `_start: () -> ()` command entry that calls the selected zero-argument Calcit `init-fn` and discards its Calcit return value.
- Host imports are selected and registered per target. The initial WASI bridge neither inherits the core target's JS `math`/`io` imports nor permits arbitrary `defwasm-import`; missing capabilities fail closed with `E_WASM_CAPABILITY`.
- Import indexing preserves the first capability registered for a module/field pair, so a user declaration cannot silently shadow compiler-owned lowering.
- Preview 1 remains an internal core-module compatibility layer. Public Calcit APIs will be modeled as capabilities such as args/env/stdio/exit without leaking concrete ABI names.
- User-visible command semantics live in definition `:tests` in `calcit/test-wasi-command.cirru`; shell checks are limited to target errors, capability errors, and Wasmtime startup.
- `--check-only` and emission share the same WASI target validation without writing a module during checks, preventing entries or capabilities from passing checks only to fail during generation.

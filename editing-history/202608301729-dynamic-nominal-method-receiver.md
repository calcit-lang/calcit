# Dynamic nominal method receiver diagnostics / Dynamic nominal method 接收者诊断

## 中文

- 修复 #527：当 postfix Option/Result method（如 `.unwrap-or`）的接收者仍为 `Dynamic` 时，预处理器不再静默回退为普通参数调用。
- 新增 `W_DYNAMIC_NOMINAL_METHOD_RECEIVER`，要求先通过 schema/assertion 收窄接收者；有意保留的 Dynamic 边界可显式使用对应 `option:*` / `result:*` 函数。
- 诊断只覆盖 Option/Result 的 nominal container methods，其他 Dynamic postfix methods 继续由现有 `--warn-dyn-method` / `analyze dynamic-methods` 策略审计，避免无关兼容性扩大。
- 补充单元测试、完整 snippet 预处理回归，以及 upgrade/polymorphism 文档。
- 验证通过：strict rustfmt/clippy、全部 Rust tests、Calcit core 223/223、native/JS/IR/WASM、Agent interface 17/17；使用新编译器检查最新 copyboard 与 calcium-workflow main 均通过。

## English

- Fix #527: when a postfix Option/Result method such as `.unwrap-or` still has a `Dynamic` receiver, preprocessing no longer silently falls back to treating the method token as an ordinary argument.
- Add `W_DYNAMIC_NOMINAL_METHOD_RECEIVER`, requiring a schema/assertion-based narrowing step; an intentionally Dynamic boundary may explicitly call the corresponding `option:*` / `result:*` function.
- Scope the unconditional diagnostic to nominal Option/Result container methods. Other Dynamic postfix methods remain under the existing `--warn-dyn-method` / `analyze dynamic-methods` policy to avoid broad unrelated compatibility changes.
- Add a unit regression, a full snippet preprocessing regression, and upgrade/polymorphism documentation.
- Verification passes strict rustfmt/clippy, all Rust tests, Calcit core 223/223, native/JS/IR/WASM, and Agent interface 17/17; the new compiler also checks the latest copyboard and calcium-workflow main branches successfully.

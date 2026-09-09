# Bare enum constructor value diagnostics / 裸枚举构造器值诊断

Issue: calcit-lang/calcit#928

## English

- Strict preprocessing now rejects a bare zero-argument `%name` enum
  constructor when an enum value is required by a definition return, an `if`
  branch, or a callable parameter. The stable
  `E_BARE_ENUM_CONSTRUCTOR_VALUE` diagnostic shows both types and suggests an
  explicit invocation such as `(%none)`.
- Equality between bare constructors of the same enum is rejected because
  comparing the same function object can hide a missing invocation.
- Explicit constructor calls and constructor symbols in an intentional `Fn`
  context remain valid.
- `query type-at` propagates a definition return expectation into `if`
  branches and recovers implicit core symbol schemas even when strict
  preprocessing stops before a source-correlated processed node is available.

## 中文

- 严格预处理现在会在 definition return、`if` 分支或 callable 参数要求枚举值
  时拒绝裸的零参数 `%name` enum constructor。稳定诊断
  `E_BARE_ENUM_CONSTRUCTOR_VALUE` 会展示实际/预期类型，并建议使用
  `(%none)` 这样的显式调用。
- 同一枚举的裸 constructor 之间不再允许直接相等比较，避免同一个函数对象的
  比较掩盖缺失调用。
- 显式 constructor 调用，以及明确 `Fn` 上下文中的 constructor symbol 保持有效。
- `query type-at` 会把 definition return 预期传播到 `if` 分支；即使严格预处理
  提前停止、无法生成对应 processed node，也能恢复隐式 core symbol 的函数签名。

## Validation / 验证

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test` (780 library, 318 CLI, and 23 WASM tests)
- `yarn compile`
- `yarn check-agent-interface` (22/22 plus definition protocol round trips)
- `yarn procs-link && yarn check-all`
- Candidate CLI against current `calcit.std`: strict check-only, zero-debt
  quality, 53/53 full type coverage, five focused examples, and native runtime
  all passed.
- Candidate CLI against `calcit.std` before its `rand-nth` fix: `query type-at`
  reports the constructor `Fn` type, inherited `Option<T>` expectation,
  `E_BARE_ENUM_CONSTRUCTOR_VALUE`, and `W_TYPE_AT_EXPECTED_MISMATCH`.

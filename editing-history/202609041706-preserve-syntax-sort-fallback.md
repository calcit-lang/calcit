# Preserve Syntax sort fallback / 保留 Syntax sort 开放路径

Address PR #632 review feedback by making the phase-aware fallback effective rather than nominal. `List<Syntax>` sort calls now replace the generic comparator expectation with the prior open `DynFn` contract; returning `None` from specialization was insufficient because the new base proc signature still enforced `Fn(T, T) -> Number`.

根据 PR #632 的 review 意见，让 phase-aware fallback 真正生效。`List<Syntax>` 的 sort 调用现在会把泛型 comparator 期望替换回原有开放 `DynFn` 契约；此前仅从专门化返回 `None` 并不足够，因为新的基础 proc signature 仍会强制 `Fn(T, T) -> Number`。

A focused regression assertion verifies that ordinary `List<String>` keeps its concrete comparator contract while `List<Syntax>` restores the open comparator. Validation: `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`, serialized `cargo test --all-targets --all-features` (689 library, 294 CLI, 23 WASM tests), and `yarn check-all` all pass.

定向回归断言验证普通 `List<String>` 保持具体 comparator 契约，而 `List<Syntax>` 恢复开放 comparator。验证：`cargo fmt --all`、`cargo clippy --all-targets --all-features -- -D warnings`、串行 `cargo test --all-targets --all-features`（689 个 library、294 个 CLI、23 个 WASM 测试）与 `yarn check-all` 全部通过。

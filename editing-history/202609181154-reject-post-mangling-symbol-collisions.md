# Reject post-mangling Component symbol collisions / 拒绝 Component mangling 后的符号碰撞

## 中文

- Component import/export 的唯一性校验改为使用最终写入 core module 的名称，而不是用户声明的原始名称。
- 同步符号若伪装成 async canonical mangling 名称，会与真实 async 符号确定性冲突并在编码前报错。
- import 与 export 分别增加回归测试，确保校验与实际 emission 共用同一名称计算逻辑。

## English

- Validate Component import/export uniqueness against names emitted into the core module instead of raw user-declared names.
- A synchronous symbol that impersonates an async canonical mangling now conflicts deterministically with the corresponding real async symbol before encoding.
- Add import and export regressions while sharing the emitted-name calculation between validation and code generation.

## Verification / 验证

- `cargo fmt --check`
- `cargo test codegen::emit_wasm`
- `cargo test --test component_wasm_cli`
- `cargo clippy --all-targets -- -D warnings`

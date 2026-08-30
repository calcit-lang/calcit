# Calx strict program lowering / Calx 严格程序 lowering

## 中文

- 在完整 eligibility graph 通过后，从同一份 typed `CompiledProgram` expression 先建立内部 lowering plan，再通过稳定版 `calx_vm 0.3.0` 的 `ProgramBuilder -> CalxProgram -> ValidatedProgram` 生成可执行程序；entry 映射为 `main`，其余 reachable function 使用确定的 fully-qualified name。
- 首批 lowering 覆盖 `Number`/`Bool`/`Unit`、typed parameter/local、单 binding `&let`、sequence/drop、完整 `if`、数值运算与比较、direct call、direct tail call 与 `recur`。每条生成 syntax 携带可追踪到 Calcit definition/tree path 的 synthetic source origin。
- 严格运行边界只做 `Calcit::Number <-> Calx::F64`、`Calcit::Bool <-> Calx::Bool`、void result -> `Calcit::Unit` 的精确转换。Nil、Dynamic 或其他 runtime value 在实例化 VM 前拒绝，不产生占位值。
- 只有 `Eligibility` 允许 embedding 选择 whole-kernel fallback；`Lowering`、`Build`、`Validation`、`Instantiate` 与 runtime trap 保持独立，执行失败后不会自动重跑 Calcit，避免未来 effect import 双执行。
- source-backed fixtures 实际执行 range sum、Fibonacci 与带 typed local/helper tail call 的 affine kernel，并与 native Calcit runner 做差分；Dynamic callee 继续验证整条 closure 在 lowering 前回退。
- 新依赖树复用了已有 `argh`、`cirru_parser`、`regex`，额外引入 `bincode` 及其 derive 构建依赖；同工具链 release LTO 产物从 main 基线 9,392,160 bytes 增至 9,392,368 bytes（+208 bytes），默认执行路径没有 Calx 初始化。
- 验证通过：`cargo fmt`、`cargo clippy --all-targets --all-features -- -D warnings`、`cargo test`、`yarn compile`、`yarn check-all`、`yarn check-agent-interface`（17/17），并完成 release binary size 对比。

## English

- After the complete eligibility graph succeeds, build an internal lowering plan from the same typed `CompiledProgram` expressions, then produce an executable program through stable `calx_vm 0.3.0` and `ProgramBuilder -> CalxProgram -> ValidatedProgram`. The entry maps to `main`; other reachable functions use deterministic fully-qualified names.
- The initial lowering covers `Number`/`Bool`/`Unit`, typed parameters and locals, one-binding `&let`, sequence/drop, complete `if`, numeric operations and comparisons, direct calls, direct tail calls, and `recur`. Every generated syntax item carries a synthetic source origin traceable to a Calcit definition/tree path.
- The strict runtime boundary performs only exact `Calcit::Number <-> Calx::F64`, `Calcit::Bool <-> Calx::Bool`, and void result -> `Calcit::Unit` conversions. Nil, Dynamic, and other runtime values are rejected before VM instantiation; no placeholder value is emitted.
- Only `Eligibility` lets an embedding select whole-kernel fallback. `Lowering`, `Build`, `Validation`, `Instantiate`, and runtime traps remain distinct, and execution failures never automatically rerun Calcit, preventing future effect imports from executing twice.
- Source-backed fixtures execute range sum, Fibonacci, and an affine kernel with a typed local/helper tail call, then compare them differentially with the native Calcit runner. A Dynamic callee still proves whole-closure fallback happens before lowering.
- The dependency tree reuses the existing `argh`, `cirru_parser`, and `regex` crates, adding `bincode` and its derive-time dependencies. With the same release LTO toolchain, the binary grows from a main baseline of 9,392,160 bytes to 9,392,368 bytes (+208 bytes), with no Calx initialization on the default execution path.
- Verification passes `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`, `yarn compile`, `yarn check-all`, and `yarn check-agent-interface` (17/17), plus a release binary-size comparison.

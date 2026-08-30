# Calx scalar eligibility boundary / Calx 标量子集准入边界

## 中文

- 为 Calx 编译目标建立第一段只读边界：直接消费真实的 typed `CompiledProgram` snapshot，不从源码文本重新推断，也不在分析失败时产生半成品 Calx program。
- 首批只接受固定参数的 `Number`/`Bool` 标量函数、`Unit` 结果、局部变量、完整 `if`、数值运算与比较、直接调用及尾位置 `recur`；`Dynamic`、Nil/absence、间接调用和未批准 host capability 都产生稳定的结构化 fallback code。
- eligibility 按入口的完整可达 direct-call closure 判断。任一 callee 不合格时，整个入口回退，并保留 definition、source tree path 与 call path；函数和 issue 使用确定排序，便于 golden 与跨仓库追踪。
- fixtures 从 Cirru source 进入正常 preprocessing，再分析不可变 snapshot；覆盖 range sum、Fibonacci、affine helper graph，以及 Dynamic callee 触发的 closure fallback。
- 当前不依赖尚未发布 `ProgramBuilder` 的 `calx_vm` git revision。等对应稳定 crate 发布后，再从同一 typed expression 降低到 `ProgramBuilder -> CalxProgram -> ValidatedProgram`。
- 验证通过：`cargo fmt --check`、strict all-target/all-feature Clippy、全部 Rust tests、`yarn compile`、Agent interface 17/17、`yarn check-all`（含 native/JS/WASM 与性能回归）。

## English

- Establish the first read-only boundary for the Calx compilation target: consume the real typed `CompiledProgram` snapshot directly, without re-inferring from source text or emitting a partial Calx program on failure.
- Initially accept only fixed-arity `Number`/`Bool` scalar functions, `Unit` results, locals, complete `if`, numeric operations and comparisons, direct calls, and tail-position `recur`. `Dynamic`, Nil/absence, indirect calls, and unapproved host capabilities produce stable structured fallback codes.
- Decide eligibility across the entry's complete reachable direct-call closure. One ineligible callee falls back the whole entry while preserving the definition, source tree path, and call path; deterministic function and issue ordering supports golden tests and cross-repository tracking.
- Drive fixtures from Cirru source through normal preprocessing before analyzing an immutable snapshot. Coverage includes range sum, Fibonacci, an affine helper graph, and closure fallback caused by a Dynamic callee.
- Do not depend on an unpublished `calx_vm` Git revision containing `ProgramBuilder`. Once the matching stable crate is published, lower the same typed expressions through `ProgramBuilder -> CalxProgram -> ValidatedProgram`.
- Verification passes `cargo fmt --check`, strict all-target/all-feature Clippy, all Rust tests, `yarn compile`, Agent interface 17/17, and `yarn check-all` including native/JS/WASM and performance regressions.

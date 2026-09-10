# Lower checked Calx programs / Lowering 已检查的 Calx program

## English

- Add a program-level compile API that atomically reuses the existing eligibility, planning, emission, build, and strict validation path for both lifecycle roots.
- Emit one validated Calx program with fully qualified function names and no compatibility `main` dispatcher.
- Preserve init/reload roles and exact strict signatures, including when both roles share one deduplicated function body.
- Execute either root by explicit named entry on one reusable VM instance, with the existing strict Number/Bool/F64Buffer/Unit boundary conversions.
- Attach one explicit typed host-capability set to the whole program; eligibility rejection and runtime traps remain distinct and never trigger mixed fallback.
- Keep runtime-mode selection, lifecycle scheduling, program caching/watch invalidation, Dynamic/Nil escape hatches, and platform FFI implementations out of this slice.

## 中文

- 增加 program-level compile API，对两个生命周期 roots 原子复用现有 eligibility、planning、emission、build 与 strict validation 主干。
- 生成一份使用完整限定函数名的 validated Calx program，不生成兼容 `main` dispatcher。
- 保留 init/reload 角色与准确 strict signatures；两种角色共享同一 definition 时只生成一份函数体。
- 在同一个可复用 VM instance 上通过显式 named entry 执行任一 root，并复用现有严格 Number/Bool/F64Buffer/Unit 边界转换。
- 整个 program 只挂载一组显式 typed host capabilities；eligibility rejection 与 runtime trap 继续分离，绝不触发 mixed fallback。
- 本切片不包含 runtime mode 选择、生命周期调度、program cache/watch invalidation、Dynamic/Nil 逃生口或平台 FFI 实现。

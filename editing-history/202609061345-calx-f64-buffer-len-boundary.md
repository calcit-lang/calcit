# Calx F64Buffer length boundary / Calx F64Buffer 长度边界

## English

Calcit exposes `&f64-buffer:len` as `F64Buffer -> Number`, and every Calcit `Number` in the current
Calx producer maps to VM `F64`. The Calx VM deliberately exposes `f64-buffer.len` as
`F64Buffer -> I64`, so emitting that instruction directly makes an apparently eligible kernel fail
later during strict validation with `expected F64, found I64`.

Eligibility must not promise a program that lowering cannot construct. Until Calcit has an explicit,
semantics-preserving representation or conversion for the VM I64 result, the producer rejects this
intrinsic as `CALX_SUBSET_UNSUPPORTED_FORM`. The VM instruction stays available, and the already
working checked-index/get path stays unchanged. This avoids an implicit numeric coercion and does not
introduce Nil or Dynamic.

The regression fixture runs the same source natively to preserve the Calcit result contract, then
asserts that Calx rejects it at eligibility rather than at lowering, construction, or validation.

## 中文

Calcit 将 `&f64-buffer:len` 公开为 `F64Buffer -> Number`，而当前 Calx producer 会把所有 Calcit
`Number` 映射成 VM `F64`。Calx VM 则有意把 `f64-buffer.len` 定义为 `F64Buffer -> I64`，直接生成
这条指令会让一个表面上 eligible 的 kernel 随后在严格 validation 阶段以
`expected F64, found I64` 失败。

eligibility 不能承诺 lowering 无法构造的程序。在 Calcit 拥有可保持语义的显式 I64 结果表示或转换
之前，producer 用 `CALX_SUBSET_UNSUPPORTED_FORM` 拒绝该 intrinsic。VM 指令本身继续可用，已经工作的
checked-index/get 路径不变。这样既不引入隐式数值 coercion，也不使用 Nil 或 Dynamic。

回归 fixture 先用 native Calcit 执行同一份源码以固定 Calcit 返回值契约，再断言 Calx 在 eligibility
阶段拒绝，而不是拖到 lowering、construction 或 validation 才失败。

# Clarify guarded fold return inference / 澄清 fold 返回推断条件

Address PR #631 review feedback by making the public documentation match the guarded native `foldl` inference already implemented and tested. The initial accumulator type is preserved only when the reducer has a concrete compatible `Fn` signature; an open `DynFn` deliberately leaves the result `Dynamic`.

根据 PR #631 的 review 意见，让公开文档与已经实现并测试的原生 `foldl` 受保护推断保持一致。只有 reducer 具有具体且兼容的 `Fn` 签名时才保留初始 accumulator 类型；开放的 `DynFn` 会有意让结果保持 `Dynamic`。

This is a documentation-only clarification and does not change runtime or preprocessing behavior.

这只是文档澄清，不改变运行时或预处理行为。

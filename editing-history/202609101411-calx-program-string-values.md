# Calx program String values / Calx 程序 String 值

Issue: [#955](https://github.com/calcit-lang/calcit/issues/955)

## English

- Added a private eligibility profile boundary: public kernel analysis keeps rejecting `String`, while whole-program analysis may admit exact typed `String` values.
- Lowered String literals, locals, branches, bindings, direct/tail calls, lifecycle roots, and exact typed host imports to calx-vm 0.5.1 `Str` values.
- Kept `Tag` and `Symbol` distinct and ineligible; runtime boundary mismatches fail before VM execution.
- Documented the `Arc<str>` to/from `Rc<str>` content copy and deliberately made no zero-copy or String-operation claim.
- Added source-backed native/Calx differential tests plus whole-program cache callback-reattachment coverage.

## 中文

- 增加内部 eligibility profile 边界：公开 kernel analysis 继续拒绝 `String`，只有整程序 analysis 可以接受精确 typed `String`。
- 将 String literal、local、branch、binding、direct/tail call、lifecycle root 与精确 typed host import lowering 到 calx-vm 0.5.1 的 `Str` 值。
- `Tag`、`Symbol` 保持独立且不合格；runtime boundary 类型不匹配会在 VM 执行前失败。
- 记录 `Arc<str>` 与 `Rc<str>` 之间复制内容的边界语义，不承诺 zero-copy，也不扩展 String operations。
- 增加 source-backed native/Calx 差分测试，以及整程序 cache 命中后重新挂载当前 callback 的覆盖。

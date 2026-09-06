# Reset diagnostic follow-up / 引用写入诊断跟进

Issue: #881.

Follow up the CodeRabbit finding on #880, related to #879. The shared
CheckContext already includes the call head when supplying the expression to
the formatter. Remove the additional reset! prefix without changing the warning
code, argument index, expected/actual types, or rejection semantics.

The regression first failed on `(reset! (&syntax reset!) 'state |wrong)` with
two occurrences. It checks exactly one W_RESET_ARG_TYPE_MISMATCH, preserves the
Number/String details, and requires a single call head in the expression.

复现并修复合并后到达的 review 意见：共享诊断上下文已经包含调用头，不再重复
拼接 reset!。新增先红后绿回归，保留错误代码、参数和类型信息，不改变类型检查规则。

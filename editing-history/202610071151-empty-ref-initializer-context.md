# 可变 Ref 的空初值需要显式 payload 上下文

## 决策（待维护者确认）

关联 #1737 剩余的 Ref 方向。本次采用默认方案：`Ref<T>` 的 payload 类型在构造时确定，`atom (Option :none)` 的 payload 槽固定为 Never；需要后续写入具体值时，在初始化表达式上写 `assert-type (Option :none) $ :: 'Option 'T`。不增加按后续写入推导 Ref 类型的规则。维护者可在 review 中改为局部约束推导。

## 依据

- Ref 是不变的可变容器，读写两侧共享同一个 T；由写入反推 T 会让类型依赖写入出现的顺序和位置，且别名、闭包与跨函数传递都要参与同一方程，属于非局部推导。
- 未推导的槽不能当作“无值”或 Dynamic（#1737 的约束）；按写入扩大类型等同把 Never 槽延迟变成开放类型。
- `loop` 参数的推导限于词法尾部 `recur`，是局部且只读的；Ref 不复用该机制。

## 改动

`W_RESET_ARG_TYPE_MISMATCH` 在目标 Ref 的 payload 含 Never 时附带修复写法，指向初始化表达式。沿用已有诊断编号；判断由 `CalcitTypeAnnotation::contains_never` 完成，与 `contains_type_var` 共用同一个遍历。

## 验证

`calcit.core/reset!` 增加带 `:reset-proof` tag 的 `:tests`，覆盖显式上下文下写入 Number 和重置为 None，并由 `check-known-assertion.mjs` 在 native 与生成 JS 回放。错误 String 写入、经别名写入及无上下文空 Ref 写入 Number 加入现有七入口拒绝矩阵，并检查修复提示只在空初值情形出现。

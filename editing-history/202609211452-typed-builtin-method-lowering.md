# 恢复类型化内建方法的静态 lowering

## 问题

严格类型模式把 List、String、Map 等内建方法表与同名 trait 实现一律当作歧义，或将内建方法表误认为用户旧式无来源 impl。之前为通过检查，把许多原本验证 `.method` 的测试改成具名函数或 native proc，丢失了方法调用的编译器覆盖。

## 调整

- 已知内建类型按 core 方法表的固定优先级选择方法，在编译期内联成直接调用；不放宽用户定义的无来源 impl，也不替多来源 trait 冲突做选择。
- 恢复类型与 WASM 测试中的 List、String、Map、Set、Number 方法调用；真实 trait 歧义继续显式使用 `&trait-call`。
- 恢复 `.rem` 类型化基准与 direct proc 对照，并验证生成 JS 的返回表达式相同。

## 验证

- `yarn check-all` 全部通过，含 native、JS、WASM。
- `cargo test -q typed_builtin_method_precedence_stays_static_without_weakening_nominal_dispatch` 通过；既有旧式 impl 拒绝测试保持通过。


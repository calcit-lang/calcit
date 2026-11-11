# 递归 union 之间的类型关系 (#2687)

- 问题：`union_relation` 比较两个 union 时逐个比较成员，成员里又引用这两个 union（`NumericTree` 对 `MixedTree`），于是无限递归。访问计数只在 `bounded_plain_type_relation` 里生效，union 关系在它之前递归，结果直接栈溢出。
- 做法：在 union 对 union 的分支加一个线程局部的假设集，键是 `(实际 union, 期望 union, 是否证明模式)`。进入时登记，遇到已登记的同一对直接按成立返回（共归纳），检查结束由守卫移除。成员中有任何一项不成立，整对仍然失败，所以不会把真正的不匹配放过。
- 只拦 union 对 union：其他路径要么一层层变小（容器拆开），要么已经有别名 / 类型槽的栈保护。
- 实现细节：守卫用 `bool::then` 构造，不能用 `then_some`，否则对已登记的一对也会先构造再丢弃守卫，把外层的登记删掉。
- 验证：`tests/strict_check_cli.rs` 的 `recursive_union_relation_terminates`，fixture 覆盖自递归、互相递归和反方向的不匹配。

# 改写后不变量校验的第一个增量（#1553）

预处理在一次遍历中同时做推断、检查与改写，改写生成的节点会被重新推断而丢失精度，或根本没有进入检查（#1378 的 `%none` 擦除 payload、#1428 的 `recur` 参数、#1494 的内联 Proc 方法）。本增量在 `src/runner/preprocess/post_lowering.rs` 增加只在 `CALCIT_LINT_CORE=1` 时运行的最终节点树校验，不改变默认构建、不新增诊断编号。

决策：

- 改写点只记录改写前类型和来源，校验在定义预处理结束后重新检查最终节点树；类型化访问记录的是再次预处理后的最终节点，而不是原始展开，避免把尚未绑定的生成临时变量当作精度损失。
- `Proc` 调用与 `recur` 的校验复用现有参数检查函数，把结果与预处理阶段已报告的警告比较；差集才是遗漏检查。`recur` 的源码检查原先对 `calcit.core` 整体跳过，校验对它不设豁免。
- 违规是带来源链的内部错误（`CalcitErrKind::Unexpected`），沿用 provenance 字段，不新增诊断代码。

校验在 core 与测试套件上发现并修复了真实问题，而不是豁免：

- `&map:keys` 等泛型 Proc 的返回类型没有用实参证据替换类型变量，内联后得到 `Set<K>`；通用的 Proc 返回推断现在替换已证明的类型变量。
- `&trait-call` 的返回推断没有把 trait 约束所要求的 trait（`requires`）计入，经 `Greeting` 约束调用 `Labeled` 的方法时返回类型退化为 Dynamic；现在与表层方法查找一样使用可达 trait 集合。
- `group-by` 的 `update` 返回 Dynamic，写回 `Map<K, List<T>>` 的 `recur` 无法证明；改为类型化 `get` 加 `option:unwrap-or` 后写入，语义不变。
- `contains-in?` 沿嵌套路径下降，每一步的值类型都不同，参数被声明为泛型 `T` 使 `recur` 无法成立；与 `get-in` 一致改为 `Dynamic`。

未完成部分：第一条不变量（每个节点有类型或明确 Unknown）、用户函数与方法调用最终节点的重新检查、宏展开与 `let` 折叠等其他改写点的记录、临时分支回退验证的自动化。

验证：新增单元测试与 `tests/post_lowering_cli.rs`；`CALCIT_LINT_CORE=1` 下运行 core `:tests`、`analyze check-public`、`calcit/test.cirru` 与完整 `yarn check-all`。

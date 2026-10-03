# 宏求值与展开结果预处理分离

## 目标与范围

#1737 的剩余问题是递归输入约束尚未求解时，第一次函数体预处理就需要选择静态方法。先为后续分阶段处理拆出宏求值：保留现有 Calcit 表示，返回原始展开代码、类型绑定、缓存 miss token 和 evaluator 的 gensym 终点；调用方继续完成展开结果预处理、合同验证与缓存提交。

这一步只调整内部职责，不解决首次方法选择的求解顺序，不增加公开命令、语法、类型或动态派发通道，也不改变未注解空 Ref 的语言策略。后续语义修复仍需要 Calcit 附带测试证明正反分支、别名与闭包捕获保持一致，不能把准备工作当作功能已完成。

## 必须保留的合同

- 初始及宏 `recur` 参数沿用原有输入合同检查。
- evaluator 沿用 capability 上下文和 assertion proof 策略；运行时生成的函数仍保留原 macro call stack。
- 纯宏 cache hit/miss/bypass 和 metrics 语义不变；带副作用宏不因这次提取被加入缓存。
- 仅在展开结果预处理与结果合同验证成功后提交 cache entry；gensym 终点来自 evaluator，而不是嵌套宏处理后的状态。
- 现有 core let/map fast path 不扩展授权范围。

## 栈回归与修复

首次直接把展开结果临时状态放进 `preprocess_list_call` 后，完整 Cargo 回放捕获 `large_case_under_a_dependency_chain_checks_without_aborting` 栈溢出：20 层函数依赖末端的 160 分支 case-default 在普通检查中 abort。初步 native/JS/core 与递归拒绝回放通过，不能证明这个大型表达式边界也通过。

沿用 #1584 已采用的职责拆分，把整个宏调用分支放进独立且不内联的 `preprocess_macro_call`；普通 syntax/函数调用不为未执行的宏状态预留递归栈帧空间。没有增加线程栈大小、缩减 fixture 或给 case-default 加特判。原精确回归恢复，包括普通检查、keep-going、首/中/尾/默认分支语义，以及错误末分支正常报告而非 abort。

## 验证与后续

```sh
cargo test --test strict_check_cli large_case_under_a_dependency_chain_checks_without_aborting
cargo test
cargo clippy -- -D warnings
CALCIT_BIN=./target/debug/calcit yarn check-all
```

现有 Calcit definition 附带测试覆盖用户语义；Rust CLI 子进程测试覆盖不可在语言内部断言的进程 abort 边界。宏缓存、capability 与 metrics 的低层测试继续保留。完整门禁最终结果写在功能 PR，不把本记录或一次定向通过替代最终验证。

后续仍需在递归约束求解前后明确准备与静态方法选择边界，保留宏单次求值和合同验证，复用当前 preprocess 表示。真正开放的 payload 必须继续拒绝具体使用；不能用宽松模式预处理、方法名特判或扩大 Dynamic 替代求解。

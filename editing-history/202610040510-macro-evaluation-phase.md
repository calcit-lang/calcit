# 宏求值与展开结果预处理分离

## 目标与范围

#1737 的剩余问题是递归输入约束尚未求解时，第一次函数体预处理就需要选择静态方法。先为后续分阶段处理拆出宏求值：保留现有 Calcit 表示，返回原始展开代码、类型绑定、缓存 miss token 和 evaluator 的 gensym 终点；调用方继续完成展开结果预处理、合同验证与缓存提交。

职责拆分不解决首次方法选择的求解顺序，不增加公开命令、语法、类型或动态派发通道，也不改变未注解空 Ref 的语言策略。本次还修复原始 Symbol 忽略词法类型的问题；后续语义修复仍需要 Calcit 附带测试证明正反分支、别名与闭包捕获保持一致，不能把准备工作当作递归约束求解已完成。

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

### 原始宏输入的词法类型

准备递归约束求解时发现同一 `Expr<String>` 宏合同拒绝字面量 `7`，却接受 `let` 绑定为 Number 的原始 Symbol。宏接收未展开 source，因此输入检查发生在 Symbol 转为带类型 Local 之前；表达式推导只查 namespace 定义，没有读取已有的词法 scope。

让通用 Symbol 表达式推导优先读取 scope，再回退到 namespace 定义。不会求值宏参数或把原始语法替换成运行时值，也不按宏名新增规则。现有集成运行器中的 Calcit `:tests` 覆盖 String 局部变量、别名、闭包捕获正例；对应 Number 输入与字面量通过既有七种检查/后端入口拒绝，沿用 `E_MACRO_INPUT_EXPR_TYPE`，并检查 source 定位和无应用产物。

此修复没有消除另一重检缺口：初始 `Option<Never>` 满足宏 `Expr<Option<String>>` 后，宏展开为常量；尾部 `recur` 推导出 `Option<Number>` 再重检展开代码时，原宏输入合同已不在代码中。这是后续分阶段处理必须保留的义务，不能把本次直接输入回归通过解读为参数重检中的宏合同完整。保留原宏 source 或合同的方案还必须保持宏单次求值、词法遮蔽、head 表达式语义及缓存/gensym 合同；不采用包裹展开结果后逐类修补 head 的方法。

```sh
cargo test --test strict_check_cli large_case_under_a_dependency_chain_checks_without_aborting
cargo test
cargo clippy -- -D warnings
CALCIT_BIN=./target/debug/calcit yarn check-all
```

现有 Calcit definition 附带测试覆盖用户语义；Rust CLI 子进程测试覆盖不可在语言内部断言的进程 abort 边界。宏缓存、capability 与 metrics 的低层测试继续保留。完整门禁最终结果写在功能 PR，不把本记录或一次定向通过替代最终验证。

后续仍需在递归约束求解前后明确准备与静态方法选择边界，保留宏单次求值和合同验证，复用当前 preprocess 表示。真正开放的 payload 必须继续拒绝具体使用；不能用宽松模式预处理、方法名特判或扩大 Dynamic 替代求解。

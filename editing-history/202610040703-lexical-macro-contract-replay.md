# 循环参数重检保留原始宏合同

## 问题与范围

#1740 在空 Enum 的循环输入收紧后重检已展开函数体，补回普通字段、assertion 与方法的类型门禁，但宏展开为常量后原始输入合同已经消失。`Expr<Option<String>>` 宏先接受空 Option，后续 `recur` 推导出 `Option<Number>` 时，检查常量展开结果不能发现错误。#1742 修复原始 Symbol 的词法类型并拆出 evaluator，本次继续补齐重检的输入、结果与泛型绑定合同；关联 #1737、#1553。

这不是首次静态方法选择前的约束求解实现。`(value .unwrap) .to-string` 的求解顺序与可变空 Ref 策略仍单独待办，没有新增公开类型、命令、诊断编号、迁移改写或动态派发通道。

## 方案

循环的参数合同更新后，重新处理原始函数 source，而不是仅重处理已经展开的 body。使用仅在当前词法编译中存活的 plan 保留已求值的宏 source、已经解析的 callee 和宏内部 `recur` 输入；在新 scope 中重新验证输入与展开结果，不重新执行 evaluator。普通函数无需此 plan；需要空 Enum 约束的局部函数启动 plan，其嵌套函数继承独立的子 frame。

这份数据是单次求值的编译产物，不是局部类型 provenance registry 或跨编译副作用缓存。类型本身仍来自现有 scope 与通用推导，原始 Calcit AST 和展开结果语义不变。没有把原始展开包进新节点再为函数、构造器等 head 逐类打补丁。

## 源码身份与求值次数

单独以源码位置或 Arc 地址为 key 不足够：宏可以把同一个 source 节点插入两次，语义上必须执行两次。plan 使用词法函数 frame、父展开、保留 source 的 Arc 身份与出现序号区分逻辑调用。source Arc 保持存活，避免分配器复用地址；函数与宏的 key 分类分离。重检时恢复同一 frame 的遍历序号，因此每个原始逻辑调用复用自己的结果。

试验还发现同一静态宏定义重新解析后会分配新 runtime ID。该 ID 继续用于原有纯宏缓存的失效检查，但不作为词法重检的身份；重检保留已经解析的 callee，避免重新编译定义或重复副作用。仅静态 Symbol/Import head 直接复用 callee；表达式 head 仍先预处理，让 head 内部的原始宏合同也得到重新检查。

嵌套函数使用子 frame，循环自身的重启使用一次性的 restart scope。独立 namespace 定义编译暂停外层 plan。所有 scope guard 在返回错误或 unwind 时恢复外层状态，不把失败编译的 plan 留给下一次编译。此内部生命周期通过 Rust invariant 测试覆盖。

## 缓存、泛型和 capability

初始参数与宏内部每一次 `recur` 都使用当前词法类型重新检查。最后一轮重新生成的泛型绑定用于展开结果合同；不保留旧 scope 的类型绑定。原有纯宏缓存也保存 `recur` 的输入 frame，warm hit 与 cold evaluation 使用相同最终绑定规则。

新增的合法宏 `recur` 正例发现原检查把 MacroSyntax 输入当作普通函数的固定值参数，报告 `unknown` 与 `unknown` 不兼容。宏递归每轮是新的语法输入/泛型实例，不能套用普通 Fn 的固定参数类型。定义处理仍检查词法 recur 位置与 arity；Macro 的输入与结果在 evaluator、缓存命中和词法重检时按同一 Macro 合同检查。普通函数及宏生成函数仍保留完整 recur 参数类型门禁，不按 macro call stack 关闭它们。

带 capability 的宏仍不进入跨编译纯宏缓存。局部重检复用本次已经通过 capability 检查的求值结果，不重放宿主副作用。core let/map 沿用已有 native fast path 与合同，没有扩大授权。缓存只在展开结果处理、验证成功后提交，gensym 进度仍来自 evaluator 终点，词法重检不重复推进 evaluator 的 gensym。

## 函数预期上下文的边界

表达式 head 正例还暴露了一项上下文泄漏：两参数 loop 的预期 Fn 签名在函数体预处理期间仍存活，宏返回的零参数匿名函数错误继承该签名，触发结果合同不匹配。这个错误不能靠改变 fixture 的参数、添加注解或放宽宏结果类型解决。

函数读取预期签名时消费该上下文，只把它应用于当前函数表达式，不让内层独立函数继续继承；原有调用方在处理表达式返回后恢复其外层上下文。显式 schema 和 body hint 仍优先，同一循环的递归重检重新提供已收紧的预期签名。表达式 head、captured 和立即调用的附带测试共同验证这项边界。

## 验证

现有 `check-known-assertion.mjs` 中使用结构化 CLI 创建 Calcit definition `:tests`，覆盖正确的循环宏输入、别名、捕获、词法遮蔽、表达式 head、结果合同、泛型 identity、同一 source 的双插入、嵌套函数与立即调用。native 与真实生成 JS 回放相同表达式；错误的循环输入及结果用既有七种入口拒绝，并检查 source 定位和无应用产物。没有增加新检查脚本。

内部 warm-cache/generic 测试验证第一轮 Number 输入、宏内部 recur 为 String 后，cold/warm 的最终 `T` 均是 String，并断言第二次真的命中缓存。该缓存状态不能从稳定 Calcit source 测试接口指定，因此留在 Rust；新增的另一项 Rust 测试只覆盖 plan pause/restart/unwind 生命周期。

```sh
cargo fmt --check
cargo test
cargo clippy -- -D warnings
CALCIT_BIN=./target/debug/calcit yarn check-all
```

继续保留完整大型依赖/case 进程回归与 Respo 的全部 74 项测试。最终门禁与下游失败集合记录在 PR，不把局部最小复现通过或本历史记录视为全部验收完成。

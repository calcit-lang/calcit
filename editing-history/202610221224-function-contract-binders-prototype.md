# 函数合同的词法泛型对应：未完成原型

本记录对应 #2190 的实现探索，尚未通过 core 与完整回归，不能作为已交付行为或发版依据。

## 已复现的问题

同一函数的 CodeEntry 使用 `C`，函数体部分 Map hint 使用 `T` 时，继承的返回值或 where
仍引用旧的变量。合法恒等函数在普通检查及 native/JS 执行通过，却被独立 proof 和完整
strict 拒绝；Countable 方法在继承 where 时也会丢失能力。同名对照没有这个问题。

## 原型方向

- 从显式重叠的 args、return、rest 通过现有 CallTypeProof 证明绑定双射，不根据变量名字、
  声明顺序或相同 trait 猜测对应关系。证明后再继承缺失字段，完整 callable 关系仍须校验。
- 源码 schema 保留原数据表示，只在函数体检查时为 CodeEntry 的量词分配词法身份。
  独立内层量词不能借源码同名或同一 trait 冒充外层变量。
- 复用 ScopeTypes 已有的 source-name → binder 映射保留 body hint 的拼写别名。
  只替换签名而不保留作用域别名，会使内层回调再次丢失原来的类型关联。
- 从已有 checked hint 的原始 Map 与已证明签名恢复别名，避免为 post-lowering 增加另一套规则或公开字段。
- 待推断的回调输出与函数拥有的刚性量词应区分；推断不能依赖是否启动独立证明。

## 验证状态与未完成部分

最初的合同对应原型让五组恒等、部分返回和 Countable 对照在普通检查、独立 proof、
完整 strict、native 与真实 JS 中一致通过；嵌套回调显示这还不完整。后续原型加入作用域
别名和 CodeEntry 词法身份后，core 自举暴露了更多既有关系，不能仅保留五组通过结果。

`deftrait` 构造 trait 时经通用 map/filter/count 回到尚未建立的 trait 运行时，形成自举依赖。
已通过官方事务将该宏的七处已知语法树 List/String 操作移至既有基础层，保留原 DSL、
检查顺序、错误与附带断言；不改普通用户代码的 trait 方法写法。

后续定位到 `%some` 的 Option payload 丢失：解析枚举声明的 payload 时错误应用了调用方
同名泛型的别名。复用 ScopeTypes，在该解析作用域重新绑定枚举自身的量词，再由已有
CallTypeProof 实例化；既不改变调用方的刚性身份，也不按构造器名字添加例外。
原内部枚举推断测试补充了同名调用方、Number 实例与刚性泛型实例的边界，类型推断模块
58 项 Rust 测试全部通过。这些 Rust 断言覆盖无法从用户查询看到的私有 binder 身份；
可观察语义另放在 Calcit 定义的测试中。

通过官方 dry-run/scoped revision 在 test-types-inference 增加两个定义、八项附带测试，
覆盖部分 body hint、合法嵌套捕获、Number/String/List 和 Some(None) 包装。基线 CLI 的
原共享运行器已实际回放 native 8/8、JS 8/8，证明原语义不是错误预期；这不是候选通过。
既有拒绝 harness 同时扩展了 CodeEntry C / body T 的四项错误捕获，尚未执行该完整部分。

当前候选已越过 `%some`，但仍在 `&list:find-last` 的 shortcut fold 返回证明失败。
无依赖隔离对照使用直接 Option 构造、相同 List<T>/callback<T> 合同，在当前 main 基线
普通检查和 native 执行通过，独立 proof 与完整 strict 均报 E_FN_RETURN_UNPROVEN，
所以这是既有合法代码的证明缺口，不应通过放宽刚性或跳过 core 来消除。该对照暂存于
本地官方事务生成的副本；后续还需把通用推断修复与对应正式回归合并验证。

必须继续修复通用关系，并完成多变量交叉改名、rest、嵌套捕获/遮蔽、能力与原拒绝回归；
还需要完整 core、宏边界、共享 backend、查询不泄漏内部身份及消费者验收。
没有删除原断言、放宽 Dynamic、添加 unsafe、增加 checker 或批准任何公共 API 删除。

这只是本地检查点，没有推送或创建编译器 PR。最终设计、命令与 CI 证据将在实际 PR 中记录。

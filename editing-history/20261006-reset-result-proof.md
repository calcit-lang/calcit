# Reset 的独立结果证明

正式 alpha.11 的 native 与 JS 已按既有合同返回写入值，但共享表达式推导没有 Reset 分支，
导致无依赖的 `Fn(Ref<Number>, Number) -> Number` helper 在完整 strict workflow 中报 unknown。
本次从实际赋值表达式解析证据，不从接收者或调用者返回声明借证；别名、泛型、名义值沿用共享关系。

目标先求值，目标或值的已证明退出保留 Never；实际不会写入的 raise 不视为 Dynamic payload。
原 Ref 写入检查、运行值、求值顺序、arity、后端支持边界和 ABI 不改。

推导暴露 `swap!` 的旧 `Expr<Unit>` 宏展开合同不实。其展开表达式按更新结果返回值，
宏阶段使用既有开放 `Expr<Dynamic>` 表达未展开语法；展开后的 Reset 仍独立推导具体类型。
这不是把业务 Fn 的返回值放宽，也不是用宏声明自证。序列化测试与生成清单按实际 schema 同步。

语义放在 Calcit `:tests`：赋值结果、别名与泛型、Option/List/Map、开放传递、一次求值、
目标先求值、异常不写入、嵌套写入和 swap 展开。既有 known-assertion runner 回放同源 native/JS，
验证 numeric global atom 的原有 WASM 路径，并拒绝八类错误/开放/unsafe/错泛型写入与返回声明。
不新增公开检查脚本、诊断编号或固定 fix。

真实 Diary 暴露的 Recollect Unit 实现错误已由模块 PR 修正并发布 0.0.55；重复的并行 PR 关闭。
语言强化不负责伪造 Unit。完整原业务 strict、正式 Calcit 包验收、稳定版与 milestone 收尾仍分别保留门禁。

# 增加单表达式 do 修复与 surface-latest-v2

- 保持 `surface-latest-v1` 的四条规则冻结，新增 `surface-latest-v2`，显式加入 `single-expression-do-v1`。
- 新规则只解包普通可执行源码中的 `(do expr)`；多表达式 `do` 仍由 `redundant-do-v1` 在已证明的 variadic body 中整理，宏与 quote/quasiquote 边界保持不动。
- 结构操作按 definition 与 source path 从内向外排序，并在具名构造器 replacement 内组合所选 `do` 规则，避免嵌套 path 失效。
- Calcit definition `:tests` 覆盖普通位置、quoted data 与 macro body；Rust 集成测试只固定 CLI preset、transaction、组合与幂等契约。
- 升级与 Agent 文档列出自动改写边界：Option/Result/Unit 契约、Dynamic 收窄、raw primitive、fallback 与 Struct unwrap 仍需类型证据或人工决定，不进入 preset。

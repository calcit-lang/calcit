# 泛型 Struct 的应用参数证据

当前正式 alpha.16 的真实 Reel 模型暴露两种相反错误：闭合模型中的混合 Db 进入具体消费者时被放行；
加入不参与 Db 的开放历史字段后，正确 updater 又被拒绝。问题由原始 Snapshot 的独立正反例复现，
不通过改缓存、删历史字段、降低 strict、unsafe 或 native call 替代来修复。

## 决策

构造器复用已有 `CallTypeProof`，各个泛型字段共享一次替换，调用方变量保持刚性并避免同名捕获。
不含声明泛型的字段仍由普通构造器检查负责，但不参与泛型参数推导。
普通兼容关系遇到已知 Struct 的应用参数时复用严格证明；裸类型或开放参数不能借目标声明获得具体证据。
既有有界类型关系快速路径与迭代路径都必须遵守此规则，不能仅收紧慢路径。

函数值、调用契约与返回契约的具名类型统一复用声明 namespace 的现有解析器，
使 alias 在字段关系之外也保持同一名义身份。不新增诊断编号、查询字段、公开入口或固定改写规则。

原始全量门禁同时暴露 `map-entries` 的回调证据退化：实际返回参数是 lexical `TypeVar`，
但 receiver-specialized 契约读取嵌入 hint 时把同一变量解析为未解析的 `TypeRef`。
该路径同样复用现有 lexical-generic hint 解析器，不修改原有 `map-entries` 附带测试或严格证明。

回调证据恢复后，原 `rigid-result` 门禁进一步暴露函数返回证明错误地将独立 lexical `U` 绑定到 `K`。
独立 producer 校验在实际结果保留自由 lexical 变量时复用已有 `CallTypeProof`，
声明方不存在可推导的 callee 变量，不允许用返回声明重绑定其他变量。沿用 `E_ERASED_GENERIC_RELATION`，
保留原诊断断言并补泛型 Struct 返回校验负例。普通检查原有泛型 wrapper compatibility 保留，
不把此处证据修复偷换为未经评估的全局兼容规则更改。

完整原测试拦住了对所有返回结果一律使用刚性关系的方案：`concat`、`&list:filter`、
`&map:empty` 的现有集合推导会留下开放成员类型，该方案额外改变了迁移校验政策。
最终只收紧实际仍有 lexical 泛型证据的关系；开放或具体结果沿用原策略，
不声称本 PR 完成所有泛型返回证明。原测试、断言、预算及集合实现均不修改。

## 验证与边界

用户可观察语义写在新 fixture 的 definition `:tests` 中；现有 Struct 宿主检查串行构造负例临时副本，
用 dry-run 与 revision 守卫检查修改，并读取 JS 拒绝产物中的真实诊断，不把任意非零退出当作通过。
统一运行器重放原表达式；native/JS 覆盖开放历史和两个 Db 类型，闭合子集同时验证 WASM/WASI。
原始 20 项 Reel 控制仍需复验，并保留 fmt、clippy、完整 Cargo/check-all、文档及真实项目门禁。

本地 core proof WIP 是独立工作，不导入、不覆盖。本修复关联 #1802 与改写后证据回归 #1553。
交付仍需要 PR review、最新 HEAD CI、精确 main 和正式包验证；不因本地测试通过关闭整版消费者任务。

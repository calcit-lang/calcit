# Agent 查询使用 EDN 结构化契约并修复 runtime-only context

关联 #1303。此前 `query type --format edn` 被拒绝；对 runtime-only `calcit.core/&list:contains?` 执行 `query context --format json` 时，定义体是 leaf 占位符，Cirru 程序格式化器要求顶层列表，导致整个查询失败。Agent 无法可靠地先查契约再修改。

现复用既有 JSON envelope 和共享 JSON→Cirru EDN 编码器，让 `query type/type-at/context/def/config`、`config show/modules/type-slots` 及现有搜索查询接受显式 `--format edn`。human 默认保持 Markdown，JSON 作为互操作选项；没有增加顶层命令或分析器。结构化查询失败仍非零退出，stdout 给出单一 envelope，诊断含稳定错误码，命令回显与日志在 stderr。`query config` 的结构化输出复用 `config show`，human 输出改为 Markdown。runtime-only 的 leaf 代码按占位符直接表示，并附 `I_SOURCE_BODY_UNAVAILABLE` 说明。

`query def` 的 FFI 元数据在 EDN 中保留原生 Map/Tag，不把 JSON 互操作编码中的 `__edn_tag` 等字段伪装成普通 EDN Map。`check-agent-interface` 增加两种格式的语义等价检查：内建类型、runtime-only context、项目定义及 FFI、表达式类型、config 与缺失项/非法类型/无效路径。通过 Calcit 自身的 EDN 解析命令验证输出是单一可解析文档；检查后 Snapshot 字节保持不变，并用会在运行时报错的入口确认查询不执行 init。模型效率变化仍需 #1302 的固定任务重复测量，本次只验证确定性工具契约。

Review 后补齐搜索命令在结构化模式下的失败 envelope；FFI 转成 EDN 前排除仅供 JSON 互操作的 `ffi` 对象，再插入原生值，避免 `:foo_bar` 与 `:foo-bar` 被错误归一化为同一键。只读测试现在实际查询会报错的项目入口定义，不再仅查询 builtin 类型。相关正反例均加入现有 Agent 接口检查。

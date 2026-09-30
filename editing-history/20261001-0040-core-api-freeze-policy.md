# 核心 API 冻结策略基础

关联 #1568、#1456、#1457、#1458。本批明确稳定承诺的对象、集中迁移的证明边界和现有 Agent 查询路径，不宣称冻结清单或 CI 门禁已经实现。

冻结对象必须来自审阅后的用户层角色和结构化 query 证据，不能按 `:internal` 过滤名义类型，也不能把全部可查询的 primitive/helper 冻结。方法接收者与 trait 派发证据独立于单个函数 schema；失败语义和 backend 覆盖依靠共享 Calcit 测试、严格负例与 host 边界验证，不靠展示字符串或新的 analyzer。

确定性映射优先，但不能把非等价 contract 变化伪装成 rename。基线更新自身不能绕过门禁；正式实施仍需要清单、明确别名移除版本、拒绝变化的负向 CI 验证与消费者回归。计划移除版本不替代 #1451 的实际退场证明。先发验证后的 core，再进行发布版模块迁移，最终完成 milestone 和成果 Discussion，避免收尾依赖循环。

Agent 紧凑 mutation contract 的既有 owner/缺陷处理条目补充按需稳定策略链接，保留原有紧凑行数门禁；digest 因正文变化而更新，必须重新验证 contract 查询与 Agent interface。复用 API 角色文档，不新增命令或并行文档 registry。

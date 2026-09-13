# 收敛当前语义规范化入口

- 将 entry 严格验证、只读分析、可证明 fix 和用户指定结构编辑收敛为四类稳定 CLI 入口。
- 明确兼容读取只处理旧表示，当前语义规范化不检查 source release，也不维护框架或版本专用语义数据库。
- 为 fix JSON 增加每条展开规则的证据来源、诊断码、生命周期和 source-version 依赖元数据。
- 固定后续 placement：verification profile 进入 `calcit analyze`，语义改写继续进入 `calcit fix`，syntax-node 格式扩展共享 mutation 输入契约。

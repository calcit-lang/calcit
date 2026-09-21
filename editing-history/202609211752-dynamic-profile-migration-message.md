# 旧动态方法 profile 的明确迁移提示

CodeRabbit 指出，通用 unknown-check 错误不应对任意未知名称推荐 `analyze dynamic-methods`，而已退役的 `:dynamic-methods` 需要明确替代正确性门槛。现在对该旧名称单独报错，提示改用 `:strict` 和行为测试；只读分析命令仅作为可选定位工具。对应 CLI 测试同时检查替代门槛与定位提示。

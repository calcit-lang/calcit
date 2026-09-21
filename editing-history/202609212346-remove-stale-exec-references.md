# 清理 `exec` 的残留引用

删除顶层 `exec` 后需搜索整份文档，而不只更新入口附近的示例。Agent 进阶指南的后半部仍有旧命令和报错描述；RFC 索引仍把已删除文件 `06-29-cr-exec-cli-builtins-rfc.md` 标为 Active。同步清理这些引用与源码注释，避免 Agent 从旧资料恢复错误入口。

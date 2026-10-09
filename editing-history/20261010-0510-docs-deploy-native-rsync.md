# 文档部署移除 Docker 预构建

main 的 Docs job 在 checkout 前拉取旧 Alpine 镜像时遇到 Docker Hub 429，Markdown 验证和部署都未运行。不能把这个基础设施失败解释为语言示例失败，或只重跑掩盖依赖。

改用 runner 已有的 rsync/SSH，保留原源文件、flags、目标目录和私钥 secret；仅 main push 的文档变更可进入部署。密钥沿用原转义换行格式，通过 stdin 进入独立 ssh-agent，退出时清理，不写入临时私钥文件或输出密钥。

原 action 禁用了 host-key checking；替换采用 accept-new，拒绝已知 key 变化，但不宣称固定服务器身份。未引入新 secret、扩大权限或在本地连接生产服务器。

验证覆盖 workflow/actionlint、Bash 语法、缺失 key、传输成功/失败和 agent 清理的隔离 mock。实际部署仍需 review/CI 后在精确 main 验证，完成前不恢复发布。

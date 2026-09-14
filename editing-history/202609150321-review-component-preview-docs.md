# 修正 Component preview 文档边界

- 按 Canonical ABI 的 `flatten_functype` 规则区分 export `canon lift` 的间接结果指针与 import `canon lower` 的 caller-provided return area；两者不是自定义约定。
- 标明 0.14.20 安装命令只在 crates.io 发布可见后使用，发布前继续固定 0.14.19。
- 把延期范围收窄为 Component WIT generation，避免误称 native Interface IR 的 WIT generation 尚未完成。

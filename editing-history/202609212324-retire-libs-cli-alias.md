# 移除顶层 `libs` 别名

0.19 的 CLI 收敛先删除顶层 `calcit libs`；公开入口统一为 `calcit docs remote-libs`。`readme`、`search`、`scan-md` 的参数与处理逻辑不变，`docs` 直接把同一个命令结构传给原有类库处理器，不再构造一份兼容命令。

同步更新升级说明、Agent 入口和 CLI 冒烟测试。注意 argh 会把裸 `libs` 当成 Snapshot 位置参数，因此负向测试使用 `libs search web`；`libs --help` 会显示顶层帮助并以 0 退出，不能作为别名是否存在的断言。生态仓库仍有旧 Agent 文档调用，后续迁移需逐仓库更新。

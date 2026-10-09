# 发布 checkout 保留 annotated tag

alpha.22 的 Publish 37898278970 在上传前失败：`Release tag must be annotated`。
远端仍是 annotated tag（对象 fa99206140e644999fd35a16a53fbd7d9fa32e09，peel 为
4323fbb3e9a3250a4b4992d9cd28c7d08d8620e9），不是维护者移动或错误创建了 tag。

实际日志显示 actions/checkout@v4 先抓取全部 tags，随后比较 tag 对象 SHA 与事件 commit SHA，
又执行 `+<commit>:refs/tags/<version>`，把 runner 的本地 ref 改成直接指向 commit。
其上游 input-helper 在没有显式 ref 时同时使用事件 ref 与 SHA；ref-helper 的 testRef 对 tag
比较未 peel 的 ref，因此触发第二次 fetch。显式 `ref: ${{ github.ref }}` 避免这条隐式 commit 路径。

保留当前事件 tag、原始 OIDC 身份、单次 checkout、annotated tag/peel 校验、精确 main CI 和全部发布门禁。
不把本地轻量 ref 当作合格的发布 tag，不改写 GITHUB_REF/SHA，不移动旧 tag，也不重跑已知错误版本。
修复通过普通 PR，后续新版本验证真实 registry 发布。alpha.22 在预检阶段停止，没有上传资产或 registry 包。

在已有 Node 发布测试中检查 workflow 的显式 event ref，并用本地临时 Git 仓库重放日志中的两种 fetch：
完整 ref 抓取保留 annotation；commit-to-tag 抓取会丢失 annotation，但 HEAD 仍相同。
这证明不能只比较 HEAD 便宣称 tag 身份完整。测试不访问网络、不上传包，不新增公开命令或 CI job。

# 发布恢复必须使用真实 workflow 源码身份

alpha.21 的恢复任务在测试、资产构建和已有 crate 身份验证通过后，被 npm registry 以 E422 拒绝。
虽然 npm 客户端读取环境变量生成 provenance，registry 还会将其中的源码 ref/SHA 与 Sigstore 证书中的
SourceRepositoryURI / SourceRepositoryDigest 核对；main 运行不能通过改写变量声明为旧 tag。
此前恢复设计只验证了客户端与本地输出，遗漏了这层服务端约束。

恢复入口仍是同一个 publish.yaml，但从已有 tag dispatch，不再从 main 选择另外的 tag。
只保留一次 checkout，workflow、脚本、源码与 OIDC 身份一致；prepare、crate、npm 各阶段都校验原始
GITHUB_REF / GITHUB_SHA，不生成另一份伪造来源环境。精确 main CI、原测试、资产验证、registry checksum、
已有版本 source SHA、channel 不回退及串行发布门禁保持不变。

旧 tag 无法接收新 workflow 修复，因此 alpha.21 保留不完整发布状态。修复合并并验证后发布新的 alpha 配对，
不移动旧 tag、不覆盖 crate，也不关闭 provenance 或改用 token。未来 tag 只在自身 workflow 可用、无需源码
变更时支持同 tag 恢复；需要源码修改就走普通 PR 和新版本。

验证使用 Node 发布边界测试和 actionlint。新增测试拒绝 main、其他 tag、错误或缺失的源码 SHA，
并以不可变环境对象验证不改写身份。另在真实 alpha.21 tag checkout 上运行预检，确认 main 身份会在任何
registry 上传前拒绝；这些本地检查不冒充实际 registry 发布成功，最终仍须新版本 workflow 与实际包验证。

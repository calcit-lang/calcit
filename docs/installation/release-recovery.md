# 恢复未完成的发布

发布仍由 `publish.yaml` 处理。crate 已上传、npm 尚未发布时，先检查原 Publish 的失败日志。
若原 tag 已包含可用的恢复 workflow，且问题不需要修改该 tag 的源码或发布脚本，可从同一 tag 恢复已有 Release：

```sh
gh workflow run publish.yaml --repo calcit-lang/calcit --ref <已有版本>
```

workflow、构建源码与发布脚本均来自同一 annotated tag。流程要求：

- Release 已存在且非草稿，预发布标记与版本一致；Cargo/npm 版本和 tag 完全一致。
- tag 对应提交的 Test、GitHub 默认 CodeQL「Push on main」都成功。
- 原始 `GITHUB_REF` 必须为该 tag，`GITHUB_SHA` 必须等于实际 checkout；main 或其他 ref 在预检阶段拒绝。
- 原测试、文档验证、标准与无 WASM 资产构建继续执行；没有只发布 npm 的跳过门禁入口。
- registry 已存在的版本必须属于同一 Git SHA。crate 还须通过 registry checksum 校验，且其 VCS metadata 不得标记 dirty；无法证明时停止，不覆盖。
- npm 预发布写入 `next`，稳定版写入 `latest`，不允许恢复旧版本时回退已有 channel。已存在且源码一致的版本不重新发布，也不重置 dist-tag。

npm 使用 OIDC 身份进行 trusted publishing，并生成 provenance（见 [npm 官方说明](https://docs.npmjs.com/trusted-publishers/)）。
registry 会核对 provenance 与签名证书中的源码 ref/SHA；不能从 main 运行后改写 `GITHUB_REF` / `GITHUB_SHA`
来声明旧 tag。认证仍使用原 workflow 的 trusted publisher，不改写这些环境变量，不关闭 provenance，也不新增 token fallback。

若必须修复 workflow 或源码，先通过正常 PR 合并修复，再按发布流程等待精确 main CI 成功并发布新版本。
旧 tag 保持不变，在其 Release notes 标明未完成的 registry 配对；新配对验证成功前，消费者继续使用上一个完整配对。

上传和 registry 可见性可能有时间差。网络错误不能当作「版本不存在」；上传后仍不可见时，流程明确失败，先核实实际状态再恢复。
只有 Publish 完成成功、crate/npm 同版本可见、Release 资产与 manifest 相符后，才算发布完成。

## 限制

- 不包含 `workflow_dispatch` 或含已知发布脚本缺陷的旧 tag，不能借 main 上的新脚本伪装为同 tag 恢复。
- 已发布内容不能覆盖；需要源码变更时，应通过正常 PR 和新版本发布。

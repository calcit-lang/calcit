# 恢复未完成的发布

发布仍由 `publish.yaml` 处理。crate 已上传、npm 尚未发布时，不移动或重建原 tag，也不为同一份源码另发版本。
先检查原 Publish 的失败日志，修复实际故障；若修复了 workflow，待修复 PR 合并及精确 main CI 成功后，
从 main 恢复已有 Release：

```sh
gh workflow run publish.yaml --repo calcit-lang/calcit --ref main -f tag=<已有版本>
```

恢复会使用 main 上的发布脚本，但构建和发布的源码仍来自指定的 annotated tag。流程要求：

- Release 已存在且非草稿，预发布标记与版本一致；Cargo/npm 版本和 tag 完全一致。
- tag 对应提交的 Test、GitHub 默认 CodeQL「Push on main」都成功；恢复脚本所在 main 提交也必须通过这两项检查。
- 原测试、文档验证、标准与无 WASM 资产构建继续执行；没有只发布 npm 的跳过门禁入口。
- registry 已存在的版本必须属于同一 Git SHA。crate 还须通过 registry checksum 校验，且其 VCS metadata 不得标记 dirty；无法证明时停止，不覆盖。
- npm 预发布写入 `next`，稳定版写入 `latest`，不允许恢复旧版本时回退已有 channel。已存在且源码一致的版本不重新发布，也不重置 dist-tag。

npm provenance 区分发布 workflow 与构建源码：workflow ref、run 和 OIDC 身份保留实际 main 执行信息，
源码 ref/SHA 则传入已经验证的 tag，避免把恢复脚本的 main 提交误写成包源码。认证仍使用原 workflow 的 trusted publisher，
不新增 token fallback。

上传和 registry 可见性可能有时间差。网络错误不能当作「版本不存在」；上传后仍不可见时，流程明确失败，先核实实际状态再恢复。
只有 Publish 完成成功、crate/npm 同版本可见、Release 资产与 manifest 相符后，才算发布完成。

## 限制

本流程不能修复已经发布的错误内容；需要源码变更时，应通过正常 PR 和新版本发布。

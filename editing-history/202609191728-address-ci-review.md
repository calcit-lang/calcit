# 处理并行 CI PR 的 review comments

- CodeQL（github-advanced-security）指出 workflow 未限制 `GITHUB_TOKEN` 权限；在
  `.github/workflows/test.yaml` 增加顶层 `permissions: contents: read`，`clippy` job 仍保留
  自己的 `checks: write` / `pull-requests: write` 覆盖。
- CodeRabbit 指出 `scripts/check-docs-md.sh` 的 `CALCIT_DOCS_CHECK_JOBS` 只校验十进制字符串，
  超大值会在 `(( ... ))` 中回绕；改为 `^[1-9][0-9]{0,2}$` 且上限 64，超范围直接报错退出。
- 验证：`bash -n`、边界值（1/4/64 接受，0/65/超长/非数字拒绝）、
  `CI=true CALCIT_DOCS_CHECK_JOBS=4 bash scripts/check-docs-md.sh` 仍输出 69/69，Ruby YAML 解析通过。

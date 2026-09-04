# 0.13.78 strict nil ecosystem evidence

Issue: #653, advancing #578.

- Added a read-only ecosystem audit command that freezes repository identity,
  checks analyzer envelope version, rejects dependency load failures, and keeps
  project nil debt separate from dependency debt and unrelated strict errors.
- Audited Respo at `d106b38a` and gen-code at `a112db30` with Calcit 0.13.77
  main `5915ccf3`.
- Respo has 49 project-local unresolved nil occurrences, public legacy
  `Optional<T>` fields, and a `map-kv` nil sentinel. Follow-up owner:
  Respo/respo.calcit#131.
- gen-code has zero project-local nil occurrences but retains one legacy
  optional stream callback parameter; its dependencies contribute 122 nil
  occurrences. Follow-up owner: calcit-lang/gen-code#13.
- Neither consumer currently passes the complete strict preflight; the first
  non-nil blockers are recorded rather than being mistaken for nil failures.
- No consumer repository was modified.

## 中文

- 新增只读生态审计命令：冻结仓库 revision，校验 analyzer envelope，依赖加载失败时
  硬失败，并区分项目 nil 债务、依赖 nil 债务与其他 strict 错误。
- 使用 Calcit 0.13.77 main `5915ccf3` 审计 Respo `d106b38a` 与 gen-code
  `a112db30`。
- Respo 有 49 个项目内 unresolved nil、公开 legacy `Optional<T>` 字段和一个
  `map-kv` nil sentinel；迁移 owner 为 Respo/respo.calcit#131。
- gen-code 项目自身 nil 为 0，但保留一个旧式可选 stream callback 参数；依赖带来
  122 个 nil 位置；迁移 owner 为 calcit-lang/gen-code#13。
- 两个消费者当前都未通过完整 strict 预检；首个非 nil 阻塞被明确记录，不伪装为
  nil 诊断或零债务成功。
- 未修改任何消费者仓库。

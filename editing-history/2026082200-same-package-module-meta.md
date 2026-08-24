# 2026-08-22 00:00

- 同 package 的应用和依赖模块现在复用应用 Snapshot 的 `<package>.$meta`，避免 self-module 迁移时把重复 meta 误判为命名空间冲突。
- 保留其它同名命名空间的冲突检查，并增加同 package meta 回归测试。

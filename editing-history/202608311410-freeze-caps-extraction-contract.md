# 冻结 caps 拆分契约 / Freeze the caps extraction contract

## 中文

- 记录 caps 当前源码、依赖、命令、storage/recovery、版本与测试边界。
- 明确 caps package version、Calcit toolchain version 和 `@calcit/procs` 版本的所有权。
- 增加无网络 CLI contract tests，冻结公开命令、显式版本读取与缺失输入失败行为。
- 关联 #546、#553，并要求独立发布验证后才从 core 删除实现。

## English

- Inventory current caps source, dependencies, commands, storage/recovery,
  version, and test boundaries.
- Separate ownership of the caps package version, Calcit toolchain version,
  and `@calcit/procs` version.
- Add network-free CLI contract tests for the public commands, explicit version
  reads, and missing-input failures.
- Link #546/#553 and require an independently published validation before core
  deletion.

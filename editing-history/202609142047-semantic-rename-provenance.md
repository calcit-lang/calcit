# 基于 resolver provenance 的原子 definition 重命名

## 背景

旧的 `edit rename` 只修改 definition key 与声明名。把它与 `query search`、`tree replace-leaf` 组合并不能形成语义重构：
文本相同的 leaf 可能是局部 binding、quoted data、另一个 namespace 的定义，或者 macro 展开后才出现的引用。

## 实现要点

- 在 preprocess 增加按需启用的 resolved source usage trace。只有 symbol 被 compiler resolver 转成精确
  `Import(ns, def)` 时才记录 target、Snapshot coordinate 与 macro origin；正常编译不承担额外收集成本。
- `calcit fix --rule rename-definition-v1 --ns <ns> --def <old> --to <new>` 复用现有 preview、revision、
  fingerprint、staged validation 与 atomic commit，不增加顶层命令或第二套 mutation 协议。
- 裸引用改写为完整 `namespace/new-name`，避免新名称在调用点被局部 binding 遮蔽；已有 alias-qualified
  引用保留 alias。旧 `:refer` 成员同步移除，空 rule 一并删除，definition rename 最后执行。
- macro expansion、macro source、quoted data、definition-attached tests、examples、schema 或缺失 source coordinate
  会使整个事务 fail closed。当前尚不能为这些 source kinds 提供相同 provenance 和写回 guard，因此不做部分迁移。
- staged Snapshot 会重新严格预处理全部项目 definitions。重复执行原命令时，旧 definition 已消失且新 definition
  存在会返回空 suggestions，保持幂等。

## 验证边界

集成测试覆盖同 namespace、`:refer`、`:as`、局部同名遮蔽、preview/apply/repeat、严格 post-check，
并验证 test source blocker 不会产生部分写入。CLI/事务协议属于 Rust host 边界，因此测试放在 `tests/fix_cli.rs`；
本次没有改变 Calcit 程序运行语义，不新增 definition `:tests`。

# 修正 fix 默认规则的操作顺序

## 背景

CodeRabbit 指出，默认运行多条 `calcit fix` 规则时，如果先展开冗余 `do`，后续位于其内部的 `tag-match` 叶子替换会继续使用旧路径，导致 staged transaction 失败。

## 调整

- 默认规划先生成叶子替换，再生成会改变树坐标的结构 splice。
- 通过 Calcit CLI 在现有 `tag-match-case` fixture 中加入冗余 `do`，让同一定义同时命中两条规则。
- 增加 CLI transaction 边界测试，覆盖默认 preview、两条操作的执行顺序、Calcit `:tests` 语义验证以及重复执行的幂等性。

## 测试分层

业务语义继续由 fixture 内定义在 `:tests` 的 Calcit 测试验证；Rust 测试只覆盖进程调用、Snapshot 原子迁移与操作路径稳定性这一 CLI 边界。

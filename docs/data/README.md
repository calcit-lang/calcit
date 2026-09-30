# 核心 API 契约基线

`core-api-0.28-scope.edn` 是已审阅的转换、解析和副作用入口的范围与失败语义；`core-api-0.28-baseline.edn` 用现有 `query def/type` 导出对应的声明证据。两者都是原生 **Cirru EDN**，不是 Snapshot，也不参与运行时派发。

本批是 **0.28.0 candidate 的部分基线**，不是整个 core 已冻结的声明。尚缺集合/谓词/其他公开 trait 的逐族审阅、兼容名的移除版本、实际 backend 支持矩阵与集中迁移例外。#1568 及 milestone 收尾继续保持开放。失败描述是人工审阅的语义记录，不是脚本自动证明；行为仍以已有 Calcit `:tests`、严格负例和 host/backend 回归为准。

## 导出和检查

普通用户仍使用已安装版本的 `calcit query def/context/type --format edn`。以下是仓库开发步骤，不新增用户 CLI 入口：

```bash
cargo build --bin calcit
yarn compile
node scripts/core-api-contract.mjs --export
node --test scripts/core-api-contract.test.mjs
node scripts/core-api-contract.mjs
```

导出只写 stdout，先审阅再更新基线；不是自动接受签名变化的命令。Node 子进程显式选择 JSON 仅作为工具桥接，持久化产物保留 symbol、tag 和 `quote` 包裹的原始 schema。泛型与 `where`、receiver 参数、返回类型、名义类型声明和 optional/rest arity 都不以展示字符串代替。`read-dir` 的 Bool schema 不代表 recursive 参数必填，以已有运行时 arity 为准。

方法先在明确 receiver 上查询，必须 `proven` 且不能是已识别的兼容入口，再追溯原始声明 schema。Number 实例用于派发检查，不把它误写为方法的唯一可用类型；泛型关系仍保存在 schema。尚为 `open/ambiguous` 的方法不会自动加入。`FfiTask/FfiResponse` 的 raw Dynamic 是现有宿主边界，不意味着业务层可以绕过类型约束。

CI 检查当前源码与基线，并对 PR 的 merge-base 基线重新比较；push 比较父提交。只改范围或重生成基线不能放行既有名字、schema、失败记录或 receiver 的变化。合法新增允许；函数体、局部参数名和方法内部实现路径不冻结。当前不提供 breaking-change 豁免，集中迁移例外需在 #1568 后续实现同一 PR 的映射与语义验证，不能手动跳过检查。

这里的测试放在 Node，是为了验证导出格式、Git 基线防绕过和契约比较这些维护工具边界，不重复添加 Rust 语言语义测试，也不新增动态类型统计 analyzer。失败描述的变化可由历史比较发现，但实现是否违反失败语义仍须由共享语义测试证明；不能把“文字没有变化”当作行为已经验证。

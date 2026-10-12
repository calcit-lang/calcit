# tree insert / swap 合并入口（#1566 阶段 B）

## 实际阻塞与根因

`tree` 组有四个签名几乎相同的插入叶子（insert-before、insert-after、insert-child、append-child）和两个只差方向的交换叶子
（swap-next、swap-prev）；`cursor apply` 已用操作名统一表达同一组动作。Agent 需要记住六个叶子名，
帮助文本和事务操作名也随之重复。`query host-procs` 除 effects-graph RFC 的一行说明（已更新）外，在仓库代码、文档、脚本与已盘点生态中无引用。

## 收敛方案

新增 `tree insert --at before|after|prepend-child|append-child` 与 `tree swap --with next|prev`。
两者在 handle_tree_command 入口、先于 cursor 引用解析与 Snapshot 写入，被规范化为原有叶子的参数结构，
因此路径、`--expect` 守卫、cursor 维护与 `edit transaction` 事务都沿用旧实现，写出的 Snapshot 逐字节相同。
未知的 `--at` / `--with` 值在任何写入前报错。

旧叶子按 patch 版本规则全部保留：帮助文本以 `[Deprecated]` 开头，stderr 输出一行迁移提示，
事务中的旧操作输出对应提示；提示在规范化之前计算，所以新入口不会触发它。
`query host-procs` 只标记弃用并说明无替代。未新增 `calcit fix` 规则：这些入口是命令行而非源码写法，没有可回写的 Snapshot 改写。

`cursor` 组合并（slurp/barf/duplicate 并入 `cursor apply`）未在本次实现，留给后续阶段。

## 验证与边界

tests/deprecated_aliases_cli.rs 对六组新旧写法在独立 Snapshot 上执行并比较结果文件、提示行数，
覆盖非法取值不写入、事务中新旧操作，以及 host-procs 提示；deprecations.rs 单元测试覆盖参数规范化与提示文案。
旧叶子在下一个非 patch 版本删除，迁移说明见 docs/run/upgrade.md。

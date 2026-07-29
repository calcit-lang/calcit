# RFC: 可验证的结构化编辑与受影响范围检查

状态：Draft
日期：2026-07-26
关联：`07-06-semantic-tree-navigation-rfc.md`、`05-12-program-diff-rfc.md`、`07-26-agent-machine-protocol-rfc.md`

## 1. 原则

Calcit 的编辑对象是 EDN tree，不是文本行。`cr edit` / `cr tree` 的安全性应来自 revision、semantic selector、subtree fingerprint、preview 与原子写入；不应把行号 patch 作为主工作流。

数字 path 依然有价值，但只代表某一个 snapshot revision 下的瞬时坐标。任何会改变同级节点的操作后，调用方必须重新查询或使用 selector/fingerprint。

## 2. 单操作契约

修改类命令逐步支持：

```bash
--dry-run
--expect-revision <opaque-hash>
--format json
--check-after
```

目标至少可表达 `definition ID + selector + expected subtree fingerprint`。revision 或 fingerprint 不匹配时拒绝写入，返回当前 revision 与候选节点；不做猜测性修改。

dry-run 返回语义 diff、计划写入、受影响 definitions 和 diagnostics。`--check-after` 先从 parse/schema/preprocess 的最小受影响范围开始，而不是隐式运行所有 target。

## 3. 多操作 transaction

新增：

```bash
cr edit transaction --file changes.cirru --dry-run --format json
```

第一版 transaction 以 Cirru EDN 为主输入：外层 list 中，每个内层 list 是一条完整的 `edit`、`tree` 或 `config` 参数序列；`--code` 后可以直接嵌入 `quote` AST 节点，不需要把 Calcit 代码转义成字符串。JSON argument lists 仅作为兼容机器输入保留。这样不复制子命令的参数与校验语义；后续只有在 operation-level precondition 确有需要时，才在兼容此格式的基础上增加 typed operation record。

transaction 可包含 tree replace、definition/import/config 改动；整体通过 `--expect-revision` 携带 snapshot precondition。执行顺序：

1. 读取一次 snapshot 并计算 revision；
2. 验证所有 precondition；
3. 在内存副本应用全部操作；
4. 校验 snapshot/Cirru/schema，预处理受影响 definition；
5. 生成 semantic diff；
6. dry-run 停止，或写同目录临时文件、flush、原子 rename；
7. 返回新 revision、每个 operation 的结果和 diagnostics。

任何失败都不修改原 snapshot。输出保存 operation ID，方便 Agent 精确重试。

当前实现进度（2026-07-28）：已加入 `cr edit transaction` 第一版，以可直接嵌入 quoted code 的 Cirru EDN argument lists 为主格式，同时兼容 JSON；支持 `--dry-run`、snapshot `--expect-revision`、human/JSON 输出、同目录 staging、最终 revision 复核与原子 rename。子命令仍作用于 staged snapshot，因此沿用已有 `edit/tree/config` 校验；失败与 stale revision 不写原文件。operation-level precondition、semantic diff 与 `--check-after` 留待后续阶段。

## 4. 受影响范围验证

```bash
cr analyze verify-changed --format json
```

它读取 Git diff 或最近 transaction result，基于 usages、call graph、schema dependency 找到直接修改项与调用者；执行可信的静态检查，并区分 `executed`、`recommended`、`not_run`。JS/IR/WASM 与全量测试先作为推荐项，避免从不可靠图自动宣称“已全覆盖”。

## 5. 验收

- stale revision 与 subtree mismatch 永不覆盖新内容；
- 失败 transaction 不改变文件；
- 成功写入可恢复地原子完成并返回新 revision；
- semantic diff 以 definition/tree 变化表达，而非整文件文本噪音；
- 并发 revision、重复 leaf、批量 import 与检查失败均有回归测试。

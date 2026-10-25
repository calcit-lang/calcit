# 被 lowering 重读的 indexed match

## 问题与依据

diary 升级到 0.31 时报 `match branch expected a 2-element list (pattern body), got: (%enum-def 'Result …)`，盖住了真正的 `case-default` 已移除告警。最小复现不需要 `case-default`：`get (match (Result :ok x) …) :a` 或 `(:a $ match …)` 都会失败。

类型化 `get` 的 lowering 把已经预处理过的接收者绑进生成的 `let`，再整体预处理一次。已知 enum 的 match 预处理后是 indexed 形式 `(match value EnumDef table)`，第二次进入 `process_match` 时把 `EnumDef` 当成分支，于是报语法错误。

## 修复边界

`process_match` 遇到 `(match value EnumDef table)` 时原样返回：分支已在第一次检查并编进表。其他 match 形式不变，不新增诊断。

## 验证

`calcit/test-enum.cirru` 的 `test-match` 加了 `get (match …) :a`，native 与 JS 均通过；修复前的 alpha.5 二进制在同一形式上报上述错误。diary 的复现现在报出 `case-default` 的迁移告警。

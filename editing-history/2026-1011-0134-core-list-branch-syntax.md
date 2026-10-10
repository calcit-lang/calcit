# core 列表分支接口与 nullable 遍历复核

PR #1942 的十二个内部列表遍历直接沿用旧宏的分支序列参数，单个表达式因此成为额外的列表。该层由 unquote-splicing 消费，不是运行时重复调用；但官方 `edit format` 仍输出整段括号，给人类和 Agent 审阅造成困难。

内部 `&list-match-internal` 改为接收一个空分支表达式、一个 `(head tail)` pair 和 rest 非空分支表达式。所有调用通过 query/tree 获取真实节点后，使用带 scoped revision 的原子事务改写。该宏仍只用于 core，主体必须是已经绑定的 List；不新增公开语法，不改变公共 `match` 或迁移规则。旧 `list-match` 在 macro 展开时将多表达式空分支放入无绑定 `&let`，将非空分支 splice 到 rest；原主体的单次求值、分支选择、顺序和返回值保持不变。

补充 Calcit `:tests` 时发现内部 nullable `&get-in` 递归调用了 Option-returning `get`，嵌套 Map 实例错误返回 none。改用既有 nullable `&get-raw`，保留原 `Optional<Dynamic>` 内部边界；公共 `get-in` 仍返回 Option。断言覆盖嵌套 Map/List、missing、nil、false 和空路径，不降低测试预期或引入 unsafe/Dynamic fallback。

后续 bot review 指出 raw List lookup 的缺失索引会抛错，已实际复现。仅在 `&get-in` 的 List hop 前检查 Number 与范围，缺失时返回 nil；合法索引和 Map 仍走 raw lookup，不扩大通用 `&get-raw` 的行为。范围判定沿用公共 `nth`，不额外用整数过滤静默吞掉原有无效小数索引错误。原测试追加空 List、边界越界、负数、非数字、嵌套缺失以及 false/0 命中断言。

原测试断言保留，并补充空路径/空集合、内部多表达式分支和公共旧宏的求值次数/未选分支。复用统一 runner 回放 native、JS、WASM 和真实 WASI。新测试遇到的 nullable 动态表示、运行时 quote、Ref 依然明确 unsupported；只有实际回放通过的 group-by、join-string 原测试才移除对应旧 parity 排除。提交前运行官方 Snapshot formatter，验证再次运行无变化。精确命令、结果和 CI 门禁记录在 PR 中。

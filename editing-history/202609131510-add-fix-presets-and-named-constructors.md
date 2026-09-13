# 增加升级 preset 与具名构造器迁移

Issue: #1063。

`calcit fix` 现在提供版本化的 `surface-latest-v1` preset，把当前稳定迁移规则展开为确定顺序，并在
JSON 报告中同时返回 preset ID 和实际 rule IDs。`--preset` 与单条 `--rule` 互斥，避免调用方误解执行范围。

新增 `named-enum-constructor-v1` 与 `named-struct-constructor-v1`：当旧 `%::` / `%{}` 原型能静态解析到
项目内 `defenum` / `defstruct` 时，迁移为直接调用具名构造器。Struct 的旧字段 pair 同时展开为直接构造器
使用的扁平 tag/value 参数。迁移跳过匿名或动态原型、quoted data、macro 定义、无法解析的依赖目标和可能被
局部绑定遮蔽的名称；所有写入仍经过 tree expect guard、Snapshot revision guard、staged strict preprocess
与第二次 preview 幂等检查。

组合 planner 会在一个顶层 guarded replacement 内递归整理已选中的嵌套构造器，并在构造器替换后才执行
`redundant-do-v1` splice，避免多个规则互相移动 Snapshot path。

review 后进一步把构造器内部重叠的冗余 `do` 直接组合进 enclosing replacement，并从后续 splice 计划排除该
source region；同时要求旧 Struct 构造字段与声明字段完整且唯一匹配。CLI 快速说明也补上 0.14.15 bridge 前置步骤。

fixture 把行为断言放在定义的 `:tests` 中，Rust 集成测试只负责验证 CLI 事务、报告协议与调用这些 Calcit
测试，延续 Calcit-first 的测试方向。

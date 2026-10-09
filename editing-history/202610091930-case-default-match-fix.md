# case-default 迁移到 match 的 fix 规则

## 依据

#1627 盘点显示 `case-default` 在本机 204 个生态仓库默认分支中仍有 260 处、分布在 87 个仓库，是分支形式收敛中迁移量最大的一项。它在 #1752 已标记 `:deprecated`，目标写法（`match` 加末尾 `_` 分支）已确定，但 `upgrade.md` 只给出人工改写。

## 实现

新增 `case-default-to-match-v1`。全部模式读作 tag、字符串、数字或 bool 时，宏自身展开为 `match item (pattern value)... (_ default)`，规则把这段展开写回源码，因此求值次数、未命中默认值和三个后端的行为都不变；判断直接复用 reader 的 `code_to_calcit`，与宏里的 `tag?` / `string?` / `number?` / `bool?` 一致。嵌套调用合并到外层的一次 `ReplaceNode`，附带 `:tests` / `:examples` 复用既有的 attached 流程。

其余情况只给 `requires-review`：模式含表达式、变量或 enum tuple（宏会走 `&case` 比较值，改成 `match` 会变成 enum 模式）；调用在 quasiquote 模板中；`case-default` 或 `match` 被局部、定义或 import 遮蔽；外层宏可能读取参数。外层宏的判断分两类：保持参数原样求值的 core macro 用白名单；其他宏读取其 `defmacro` 源码，只有承载该调用的参数仅以 `~p` / `~@p`（含紧凑叶子写法）拼接、rest 参数另允许 `count` / `empty?`，且拼接位置外层模板头都是函数、语法或白名单 core macro 时才通过。这让 Respo 的 `defcomp` 可以自动迁移，而 `->` 这类会改写参数位置的宏和 `quasiquote $ quote ~form` 这类会把参数当数据的宏保持 review。为此 `definition_head_is_macro` 拆出返回宏源码与所在文件的 `definition_head_macro_code`。

## 验证与边界

`tests/fix_cli.rs` 覆盖字面量、嵌套、表达式模式、`->`、转发宏、quote 宏、quoted data 与附带测试，包含预览不写入、revision 守卫应用、应用后测试通过与重复预览只剩 review 项。Timegrass 副本上按 namespace 预览与应用，3 处改写通过 staged 严格校验，enum tuple 模式保持 review。

规则不加入 preset；已知活跃下游默认分支清零后，与 `case-default` 一起在下一个非 patch 版本退场。`case` 的表达式模式、`list-match` / `struct-match` 等其余收敛项等 #1627 中的方案确认后再做。

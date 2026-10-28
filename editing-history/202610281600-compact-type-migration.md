# 类型签名统一为 quote 精简写法（#2450 阶段 A、B）

精简写法补齐了 `Macro` 合同（位置参数为必填，`& T` 为剩余，`(-> T)` 为展开结果，
`:optional`/`:capabilities` 子句）和只标记异步的 `Async` 标记。
`deftrait` 方法类型与 `defenum` 载荷类型在读入代码时展开，宏和按源码解析 trait/enum 的路径
看到的仍是规范形式，因此不需要分别修改。

Snapshot 的 `:schema` 改为 `quote $ Fn …` 存储，单个类型名存为 `quote Dynamic`。
写出前把精简写法重新展开并与规范 EDN 比对，不等价时退回规范形式；revision 哈希仍按规范 EDN 计算，
所以单纯换写法不会改变定义的 revision。build.rs 对精简 quote 原样放入二进制快照，由运行时加载器展开。

`calcit edit format` 先做原有的旧 tag 规范化，再把 `assert-type`、`unsafe-coerce`、`hint-fn`、
`defstruct`、`defenum`、`deftrait` 中的规范类型改写为精简写法，同样逐一比对。
值引用（如 `:: 'List Person`）、只写部分字段的 hint（`{} $ :args …`、`:async false`）保持原样，
留给阶段 C 决定。仓库内全部 snapshot 已用新版 `edit format` 迁移；`examples/wasi-http-client`
由已发布工具链检查，仍保持旧写法。

## 阶段 C：关闭旧写法

严格加载与 scaffold 架构计划拒绝能等价写成精简形式的旧写法，错误码 `E_LEGACY_TYPE_SYNTAX`，
信息里给出精简写法并指向 `edit format`。编辑命令的输入仍可用旧写法（脚本和 agent 大量以 JSON-AST 拼出
`:: 'Fn` 形式），写回时代码按 `edit format` 的规则改写，schema 本来就按精简写法存储，`edit schema`
另给 `W_LEGACY_TYPE_SYNTAX` 提示；fix 规则生成的 `edit schema` 操作直接用精简写法。所以文件里不会再出现旧写法。检查方式是把原始 `:schema`
与代码按存储/迁移规则重写一遍：只要会被改写就算旧写法，所以值引用、部分 hint 和 quote 里的数据不受影响。
`edit format` 走单独的宽松加载器，仍是唯一的迁移入口。

存储比对原先靠重新解析，`Result` 等名字会按当时的注册表补成 `calcit.core/Result`，导致结果随加载顺序变化；
现在先做结构比对，只有类型引用向量这类等价编码才退回重新解析。type slot 展开为 `'*name`，
Cirru EDN 才能读回；具名类型引用也存为 `quote app.main/Person`。

`examples/wasi-http-client` 仍按已发布的 0.18.0 工具链检查，保持旧写法；用当前 CLI 的测试先复制一份并
`edit format`。仓库内其余 snapshot、文档片段和 Rust 测试片段都已迁移。


`hint-fn` 后面只能是函数类型，没有歧义，所以去掉外层 quote：`hint-fn $ Fn Number (-> String)`、
`hint-fn Async`、`hint-fn $ Async (:features :js-ffi)`，两参数的 `hint-fn f $ Fn …` 也一样。带 quote 的写法仍可读取，
`edit format` 改为不带 quote。`assert-type` 等位置的类型参数可能与普通表达式混淆，仍写 `quote $ …`；
漏写 quote 的 `assert-type xs (List String)` 原先被当作未知形式静默变成 `Dynamic`，现在报 `E_TYPE_EXPR_SYNTAX`
并给出 `quote $ List String`。

测试代码原先只做精简改写、不做旧 tag 规范化（为了保留比较新旧写法的测试）。旧写法已经关闭，
测试代码与普通代码走同一套迁移和检查，core 测试里残留的 `fn (x) :number x` 和 `(.label :fn)` 一并改掉。
`validate_schema_for_write` 和 `hint-fn` 的错误信息改为给出精简写法的例子。

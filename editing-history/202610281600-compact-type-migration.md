# 类型签名统一为 quote 精简写法（#2450 阶段 A、B）

精简写法补齐了 `Macro` 合同（位置参数为必填，`& T` 为剩余，`(-> T)` 为展开结果，
`:optional`/`:capabilities` 子句）和只标记异步的 `hint-fn $ quote Async`。
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

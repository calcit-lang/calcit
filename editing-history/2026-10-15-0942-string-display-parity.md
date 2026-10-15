# 统一直接值与容器中的字符串展示

Rebase Hell 的实际 updater 测试在 native 和 JS 上通过数据断言，但原足迹日志中的
`owner/repo` 出现引号差异，阻断统一回放。正式 alpha.3 可独立复现，见 #2074。

JS 的直接 `format-to-lisp` 已按 Cirru EDN 字符分类判断是否引用，容器展示却使用
只检查空白、括号和引号的另一路径。将两处收拢到既有 `toString` 的 escaped
String 分支，并沿用 native 的字符分类和 `char::escape_default` 转义。原始字符串、
序列化协议、容器内容和业务日志调用顺序不变，不增加公共入口。

验收放在 `format-to-lisp` 定义的 `:tests`：直接值、List、Map 和 `str` 的容器展示
覆盖斜线、下划线、空白、引号、反斜线、控制符、中文及非 CJK Unicode。
使用既有跨后端 runner 验证断言与日志 trace，并重放 Rebase Hell 原六项测试。
不修改已创建的 alpha.4 tag，不增加忽略项；正式消费者验收仍须等待修复发布。

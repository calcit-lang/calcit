# Lisp 表示与诊断表示的边界

## 决策

`format-to-lisp` 使用代码形式的 Lisp 表示，`to-lispy-string` 使用 Calcit 值的诊断表示。两者对列表和 quote 不等价，因此不进入等价改名或自动 fix。面向用户的数据序列化仍优先使用 Cirru EDN；标量文本转换使用有 ToString 证明的 `to-string`。

## 兼容与验证

本次不改变 native 输出。JS 原先把简单 String 也无条件写成带引号的形式，导致 List 的 `format-to-lisp` 与 native 不同；JS 复用已有的 Calcit String 表示函数，并对非简单 ASCII 使用 native 的 Cirru EDN 字符边界加引号。`calcit.core/to-lispy-string` 的 definition `:tests` 固定列表与 quote 的不同输出，JS runtime 回归验证两种表示、带空格及下划线的 String。运行 core definition tests、JS 检查和文档检查；不将这两个入口之一隐藏或移除。非 ASCII 的全部 Unicode 分区规则仍沿用现有 JS 表示，不在此批次复制 Rust 字符表。

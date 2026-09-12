# 空集合不构成 Dynamic 类型证据

## 背景

严格类型检查会阻止 `Dynamic` 擦除函数签名中的泛型关系，但空集合字面量没有任何元素可用于推断 key 或 value。此前 `({})` 被暂时表示为 `Map<Dynamic,Dynamic>`，因而在传给 `merge`、`merge-dynamic` 等泛型函数时被误判为真实的 Dynamic 输入。

## 调整

- 泛型关系检查识别期望 Map 位置上的空 Map 字面量，不把其中的临时 Dynamic 当作擦除证据；List 的对应公开路径原本正常，本次不扩大范围。
- 只豁免当前调用位置上的空字面量；一旦空 map 先绑定为局部变量，其静态类型仍是 `Map<Dynamic,Dynamic>`，继续接受严格检查。
- 在 `calcit.core/merge-dynamic` 的 definition `:tests` 中覆盖空 map 位于首参数和后续参数两种顺序，让 Calcit 用户代码直接表达预期语义。

## 语法复核

Issue #992 中多行 `if` 的第三个分支写成独立的 `(assert-type ...)`，Cirru AST 会因此多出一层 list，语义是调用 `assert-type` 的结果。正确写法是不加整行外层括号。该现象不是类型编译器缺陷，本次没有为它增加特例。

## 验证

- `merge-dynamic` 对空 map 位于前后两种顺序均通过严格预处理并返回预期结果。
- 普通 `merge` 与空 map 的组合恢复通过。
- 先把空 map 绑定为局部变量后，`E_ERASED_GENERIC_RELATION` 仍会阻止未经证明的 Dynamic 泛型关系。
- `calcit.core/merge-dynamic` 的 3 个 definition tests 全部通过。
- 同步 core Dynamic 分类的生成基线；位置总数（281）和定义总数（194）均未变化，只有源文件 revision 更新。

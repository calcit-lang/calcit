# 字符串 Option lowering 保留存在性证明

## 实际阻塞与根因

Timegrass 历史任务排序对 String 调用 first，再用 Option 的默认值处理空字符串。
正式 0.29.0 的 callable-contract-proof-v1 会拒绝这段源码；最小 first-text 定义也复现，
源码没有断言，错误来自编译器 generated_present_string 插入的 assert-type String。
上层分支检查了非空或索引范围，但 nullable primitive 的返回类型本身没有独立存在性证明。

## 收敛方案

移除生成的断言，将字符串 primitive 结果保存到一个新鲜局部绑定，再通过普通 nil? 分支构造 none / some。
复用原有 lexical narrowing，不新增类型关系、诊断或公开入口，也不把编译器生成代码免于检查。
first 不再先检查字符串为空，而直接处理 primitive 的 nullable 返回；nth/get/last 保留原索引/数量守卫，
因此非法索引的调用顺序与错误行为没有通过捕获异常或默认值改变。
外层接收者和索引仍按原来的临时绑定保证从左到右各求值一次。

这项转换只作用于 String。List/Enum 的 nil 元素仍通过其位置存在性产生 some nil；
primitive 的 nullable schema、用户源码断言门禁与 Dynamic 边界不变。

## 验证与边界

在既有 fixture 添加 Calcit 附带测试：first/last/nth/get 的空值、越界、Unicode scalar，
日期排序、List 的 present nil，以及带副作用的参数求值顺序；既有 proof runner 回放合同正例，
拒绝未守卫的字符串 primitive、开放 List 成员和 Dynamic 值上的 String 断言。
测试在修改前重现 E_ASSERT_TYPE_UNPROVEN，修改后最小证明通过。

日期排序的 WASM 全集合断言在修复前后的构建都失败，native/JS 通过；原因仍待进一步定位，
独立记录于 #2017（Later），仅为该完整断言设置 parity 排除，保留原断言与输入。
直接 String 访问和 List present-nil 在 native/JS/WASM 都回放；副作用测试在 native/JS 回放。
原 Timegrass 完整验收还依赖 #2014 与 Respo/应用边界迁移，不以此最小修复宣称已完成。

## 成本

不新增依赖、持久状态或分析器。生成 String 的 nth/get/last 多一个局部结果绑定和 nil 分支；
first 用同一结果分支替代先前的 empty? 分支。编译和运行成本随 PR 的命令证据记录，不宣称性能提升。

# usages 复用已有词法作用域遍历

`query usages` 已能按 namespace require 识别 refer、alias、全限定与隐式 core 名称，但逐 leaf 匹配仍将参数、let 绑定及局部读取误报为全局引用。Agent 不能据此判断旧 API 是否可以删除。

本次让 usages 和 context 的引用收集复用局部 rename 的 source walker。查询按当前 namespace 解析绑定形式，处理参数、顺序 let、并行 loop 初始化及 &let；局部遮蔽只影响未限定的名字，退出作用域后恢复。绑定形式的操作符自身仍是引用；若局部 callable 恰好名为 let，不把它的实参误当成绑定列表。schema 保持单独的类型引用处理。

查询不运行用户 macro，也不承诺完整展开后的引用。quote/quasiquote 的数据不作可执行引用处理，但 quasiquote 的 `~` / `~@` 逃逸表达式按现有编译器语义在外层词法作用域执行，必须保留其中的引用（含紧凑 leaf 与嵌套模板）。紧凑写法返回原 leaf 坐标，不伪造 reader 展开的子路径。未知 macro 的参数保留源级候选；正、零结果都明确提示不能单凭查询证明安全删除。未新增查询字段、命令、固定改写或 proof 子系统。局部 rename 继续使用原有宏证据和保护逻辑。

测试保留在现有 CLI host harness：验收的是输出 envelope、namespace 归属、精确 source 坐标和候选提示，而不是 Calcit 的值级求值语义。已有 alias/schema 断言保留，增加同名局部绑定、全局初始化读取、作用域恢复、局部语法同名 callable 和结构化 context 回归。fixture 仅通过 dry-run 与 revision 守卫的官方 transaction 写入；原四个 namespace 与十一个定义的完整 query envelope（含 schema/tests/revision）保持相同。

# 附带 source 的等价 core 名称改写

## 问题与选择

同一 `join-str` 调用在 definition `:code` 中能够生成安全迁移，放在 `:tests` 或 `:examples` 中却没有建议。
第一步复用已有的 synthetic fn、编译器 usage trace、source coordinate 和 `ReplaceTest` / `ReplaceExamples`，
让 `core-function-alias-v1 --include-attached` 覆盖附带执行表达式，不增加命令或运行时 API。

原 attached resolver 服务于项目定义重命名，需要补充 core 短名的隐式可见性；命名空间定义和 import 优先，
lexical binding 的最终判断仍由编译器完成。不会通过 leaf text 直接生成自动改写。

## 保持的边界

四对函数形式兼容名的直接调用保持参数求值与结果；它们并非相同函数身份，因此一等值不能自动改名。
引号和未知宏仍需人工确认。已核实保持执行的 core 宏复用现有边界规则，不放行一般宏。
同一 metadata 区域的多个 alias 先合成一次改写；区域内存在 blocker 时整个区域不自动写入。

默认扫描范围不变，revision、VCS 与 staged validation 继续生效。当前只支持显式 alias 规则，其他规则、
upgrade workflow 与弃用元数据仍属于 #1696 未完成范围，不能据此宣称 issue 已交付。

## 验证

Calcit attached tests 验证嵌套两个旧名调用的结果及一等函数身份；CLI 协议测试验证 code-only 默认、
附带预览、quoted review、局部遮蔽警告与不改写、原子应用、重复预览和 stale revision 不写入。
Rust 测试仅负责命令协议，实际表达式断言保存在测试创建的 Calcit definition `:tests` 中。

## 共用方法证明入口

进一步将现有方法等价改名的单 source 入口提取出来，definition `:code` 与附带表达式调用同一接收者类型、
方法实现及 source origin 证明。复用已有 alias contracts，不另建 API 表、不把临时定义写入全局程序。

局部 `let` 的最小回归揭示原 trace 只返回调用证据而丢弃处理后的树，无法回收局部 receiver 的类型。
现在同一预处理返回处理后的源码与原调用证据，definition-only 接口仍保持旧返回形状。
reader 的 `@ref` 则要保留原形状保护：源码叶代表 deref 调用，不可误用其中的 Ref 类型替代最终 receiver。

附带回归覆盖 List、Map、Set 的方法改名和 FsPath 写入错误；断言在改写前后原样执行。
未知宏与开放 receiver 保留 review，quoted data 不改写；含 blocker 的示例区域整体保留。
剩余构造器、转换、body 整理等规则、preset/workflow 组合和弃用元数据仍未完成。

谓词迁移继续复用相同入口：`some?` 的已解析引用使用已有 attached resolver；`round?` 的 reader
调用头与普通 `:code` 共用 source proof，Number 方法仍由原方法证明处理，不将整个规则缩成只有方法别名。
附带断言同时覆盖 nil、非 nil、整数、小数和 Number 方法；未知宏与 quoted data 不能借 reader
解析取得自动改写权限。剩余规则与弃用元数据继续保留未完成状态。

检查定义后确认 `some?` 与 `non-nil?` 同样是两个独立 Fn，不能因函数体相同而改写一等身份；
共享规则现在只自动改直接调用，普通 `:code` 和附带区域都保留该保护。补充的嵌套 recur 负例
保证外层独立退出不为内部递归循环提供具体返回证明；原 Diary patch helper 的只读审计通过且校验和不变，
这些局部检查不代表完整 Diary 迁移。

附带 `do` 整理直接复用已有路径收集与结构化 body rewrite，synthetic fn 仅用于编译器来源证明，
不把附带根表达式误当成函数 variadic body。单表达式根可解包，多表达式根必须保留顺序容器。
未知宏沿用既有执行上下文门禁，整个 metadata 区域进入 review；quoted data 原样保留。
顺便对齐已有 splicing helper 的 `let[]` body 支持与路径收集规则，避免同一规则两处不一致。

Option/Result 旧 helper 构造器提取共享 source 入口，普通定义与附带表达式复用同一 compiler usage、
arity、类型名遮蔽和嵌套 replacement 证明。已明确保持一次执行的 fn/do/assert= 复用现有宏边界契约，
未知宏不放行。结构化子树替换继续核对原 AST，多个构造器合成原子 metadata 操作。
语义回放覆盖四个 helper、嵌套 Option/Result 与状态写入一次；局部 Option 遮蔽本来就会被编译器拒绝，
因此负例单独验证改写前后仍拒绝，不把它改成成功断言，也不扩大 Dynamic。

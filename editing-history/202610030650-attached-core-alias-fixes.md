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

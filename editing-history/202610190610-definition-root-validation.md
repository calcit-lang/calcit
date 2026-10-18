# 具名定义的根节点校验

Doctalk 迁移暴露的额外 List 包装使 Fn schema 与源码实际初始化行为脱节：普通检查和完整 strict 均通过，JS import 却立即以零参数调用带参函数。纯函数 fixture 独立复现，且只规范根节点后 strict、import 和原断言通过，不需要运行宿主或业务服务。

复用结构化编辑器已有的具名声明 head 清单，在 Snapshot 层集中判定“根节点的计算操作符包含具名声明”。预处理在 source definition 入口应用同一规则并保留 Syntax 类别和 root source location；不修改函数体、macro 展开、runtime 或 backend。普通声明不转换 AST；只有计算调用头的根节点进入该检查。tree 各写入点共用一个保存函数，只检查正在编辑的定义，不把整个旧项目变成不可修复状态。原 edit def 同样复用规则；query、loader 与正常旧源码读取不因此关闭。

错误发生在定义本身、无法由 definition :tests 的合法函数体构造，因此失败回归放入既有 strict-default 脚本和通过官方 guarded CLI 生成的历史形状 fixture；不增加 checker 脚本或 fix planner。Rust 仅验证低层源码形状及匿名调用、quoted data 和函数体不会被误判，用户入口回归仍经 Calcit CLI。非 patch 诊断收紧属于 0.31.0，不回移至 0.30.1 维护线。

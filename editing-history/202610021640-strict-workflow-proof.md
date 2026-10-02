# strict workflow 接入共同类型证明

## 依据

同一 Snapshot 中，Dynamic 参数被 `assert-type` 标成 Number 后返回，独立审计报告 `E_ASSERT_TYPE_UNPROVEN`，旧 strict workflow 却仅运行 surface 迁移及普通检查。工作流的完成状态不能替代缺失的证明。

## 决策

复用 compiler-review、既有诊断和 suggestion/manifest，每个定义只执行一次共同 proof，不运行多套检查器。独立规则保留原 scope 行为；项目 workflow 对缺少位置或依赖 owner 的错误只输出诊断，不猜写回路径。重复依赖诊断去重。

安全迁移仍先读取完整 legacy 编译证据，不受后续 proof 首错中断影响。preview/apply 有证明错误时保持 requires-review，verify 失败；apply 可提交已验证的安全迁移，但不得宣称待审证明已解决，恢复 revision 和退出信息必须反映真实写入。

Respo 回归发现，legacy 模式仍会拒绝确定矛盾的断言。workflow 在证据收集阶段按定义保留这些编译器错误并继续收集其他定义的 warnings，与 proof pass 的诊断去重后输出同一个 manifest；不把错误变成成功，也不影响独立规则的 fail-fast 行为。

集合构造是存储而不是具体类型消费：其参数证明使用既有推导得到的共同元素类型。开放集合不能借返回声明变成具体元素集合。Macro source trace 与普通编译统一，在语法构造阶段不借运行时证明；展开后的函数仍接受运行时证明检查。

## 验证

使用定义附带的 Calcit 测试验证开放集合保存值。CLI 回归覆盖 preview/apply/verify 的拒绝与无写回、独立修复后恢复通过、原生 EDN 输出、具体集合返回拒绝，以及旧 surface 迁移可继续应用但不能隐藏 core helper 或未类型化 FFI 的证明错误。旧 fixture 原有的安全改写、逐 entry verification、FFI 候选断言保留，不靠删除失败案例通过。

本步不宣称 core helper 全部已获证明，也不关闭跨 helper 最早来源、metadata synthesis 与 Ref alias 的既定验收。

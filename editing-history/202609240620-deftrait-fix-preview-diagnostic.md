# 全项目 fix 预览中 deftrait 形状错误的诊断

关联 #1314。Respo Feather 的 `FeatherIconsHost` 在快照中把方法项多包了一层列表：有效形状是 `(:icons 'Dynamic)`，实际 AST 是 `((:icons 'Dynamic))`。入口 `--check-only` 不检查未进入调用闭包的该定义；全项目 `fix --preset surface-latest-v2` 会预处理它。旧 `deftrait` 宏以 `assert` 检查方法项，失败分支使用 `eprintln`，在无 `:log` 编译期权限时先报内部 capability 错误，遮住真正的源代码问题。

`deftrait` 改为在形状错误时直接 `raise`，保留空能力集，不扩大宏权限。`fix` 的预处理错误附上正在处理的 `namespace/definition` 和已有错误位置。现在 Feather 的全项目预览非零退出，并明确报告 `feather.core/FeatherIconsHost` 与多余嵌套的项；局部 `feather.comp.container` 预览仍成功。有效 external-object trait 的全项目预览成功，非法声明失败且不写回 Snapshot，均有 CLI 回归。

这里没有自动修改消费者的业务快照，也没有将不合法结构当作有效 trait 接受。若消费者要继续全项目迁移，应通过 Calcit 编辑命令去掉该方法项的多余列表层，然后重新运行严格检查与 fix 预览。

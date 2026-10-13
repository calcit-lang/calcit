# 项目包元数据修复入口

Phlox guidebook 的旧 Snapshot 将项目标为 `docs-workflow`，实际本地定义和入口却是
`app.*`。官方恢复与格式化保留了这一历史元数据，随后编辑接口按包前缀拒绝修改本地定义。
关联实际迁移阻塞：#1987。

在既有 `config set` 中支持项目级 `package`，复用 writer lock、工具链版本守卫、
Snapshot serializer 及编辑事务。包名使用非空 symbol 段，拒绝 entry 选择器。
只修改根 Snapshot 的包名；不加载或写入依赖，不重命名源码、引用或入口。
原有 namespace ownership 检查保持不变；事务 scope 已包含项目 package，无需新 revision 类型。

CLI 文件边界回归覆盖错误输入无写入、dry-run、项目 scoped revision、恢复本地归属、
依赖编辑拒绝与事务失败回滚。真实项目验证在 guidebook 的临时 Snapshot 副本上完成，
13 个原有定义的 query 数据在修复前后完全一致，本地定义编辑 dry-run 成功；
检查不再报告 docs-workflow 的未知导出，继续暴露旧 API、FFI feature 和类型合同欠缺。
待正式工具链发布后再应用到迁移分支。本次不新增 fix rule、诊断编号或检查脚本。

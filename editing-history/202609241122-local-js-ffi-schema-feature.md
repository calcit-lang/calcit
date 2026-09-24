# 局部增加 JS FFI schema feature

## 决策

`CodeEntry :ffi` 描述导出或宿主元数据，不是函数体的词法权限。继续让严格预处理只识别结构化 `Fn` schema 的 `:js-ffi` feature，不引入 namespace/entry 级授权，也不把 `E_UNSCOPED_UNSAFE_COERCE` 降级。

为了避免 Agent 重写长 schema，在现有 `edit schema` 下提供显式 `--add-feature js-ffi`。该操作只接受已有 `Fn` 签名，保留参数、返回值、泛型、约束、其他 feature 及 CodeEntry 其余字段；缺少签名时拒绝猜测。命令重复执行不写盘。需要 revision 前置条件和多步原子提交时，将它放进现有 `edit transaction`，不新增迁移入口。

## 边界与验证

CLI 选项目前只允许 `js-ffi`，避免无意接受尚无语义定义的 feature。诊断明确指出 `edit ffi` 与 `edit schema` 的不同用途。Rust CLI 测试覆盖字段保留、幂等、过期 revision、dry-run 与事务提交；用隔离的 `calcit/add.cirru` 副本验证了 EDN 预览、带 revision 提交和重复执行。宿主值的有效/无效运行时契约以及 Timegrass 真实组件迁移仍属于 #1285 后续验收，不能仅凭 feature 修改宣称完成。

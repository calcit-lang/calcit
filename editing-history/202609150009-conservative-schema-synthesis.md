# 基于现有推导事实合成保守 schema

- 新增显式 `calcit fix --rule synthesize-schema-v1 --ns <ns> --def <name>`，继续复用统一 preview、revision、fingerprint、staged validation 与原子事务入口，不加入升级 preset。
- compiled definition 只暴露实现中已经存在的 bottom-up 类型结果；schema planner 用同形状合并填补 `Dynamic` 洞，不另建递归推导器，也不执行程序猜测运行时值。
- 普通项目源码中的参数证据通过 resolver trace 收集；只有所有可定位静态调用点的对应实参类型一致、且不存在 macro 定义/展开、函数值、缺失坐标或调用形态冲突时才进入候选。tests/examples 不作为公共参数签名的证明。
- tests/examples 虽不参与参数类型合成，staged transaction 仍会在新 schema 下严格预处理其中引用目标的表达式；冲突样本拒绝整个候选，避免把“不是证明”误解成“可以忽略消费者”。
- 完整候选才生成 `edit schema` operation；部分候选保留 `schema.args.<index>`、`schema.return...` 等精确 unresolved path，标记为 `needs-review`，即使传入 `--apply` 也不写回。
- 推导出的 nominal Struct/Enum 值先恢复为 source-level TypeRef，避免序列化成宽泛 `Struct`/`Enum`。已有 generics、`:where`、features、函数类别和已知 slot 保持不变。
- schema 合成实现拆到 `fix/schema_synthesis.rs` 内部模块，公开入口仍只有 `calcit fix`，避免继续膨胀主命令分派文件。
- CLI 集成测试覆盖零参数函数、普通参数调用点、`Ref<Number>`、嵌套 partial hole、冲突调用点、幂等、原子写回以及 native/JS 候选一致性。
- 发版前审阅同步把升级指南中的当前 runtime/compiler 安装版本更新为 0.14.19；0.14.15 migration bridge 等历史说明继续保留，因为它们仍是老项目跨版本升级所需的有效路径，不属于过期入口。

# 可选参数诊断加入解析后的项目调用证据

## 决策

延续 `optional-parameters-v1` 的只读边界，复用编译器现有的 `trace_definition_source_usages`，而不维护一套按名字猜调用者的扫描规则。预览现在记录项目定义源码中的解析引用，并区分直接调用、spread 调用、函数值与 macro 来源；直接调用显示实参数量和显式 `nil` 位置。扫描在迁移兼容模式下进行，不放宽普通 strict 编译。

## 安全边界

扫描失败的定义必须显式出现在证据中；definition-attached tests/examples 和仓库外消费者尚未构成完整闭包。结果仍统一为 `needs-review`，`--apply` 继续拒绝。这些数据只为后续判断原子改写可行性提供事实，不把已发现的部分调用点误报为“所有调用点已证明”。

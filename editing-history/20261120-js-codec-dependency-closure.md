# JS 类型化 codec 的依赖闭包

## 复现

#2886 中单独选择 Quatrefoil select-keys 的测试，编译成功后 Node 因缺少 quaternion.vector.mjs 失败；#2863 报告同类混合 namespace 问题。最小 fixture 只运行算术入口，同 namespace 的未调用 decoder 引用另一个 namespace 的名义 Struct，仍产生同类缺失导入。

## 原因和决策

JS 沿用可达 namespace 完整输出策略。补齐同 namespace 定义会产生 DataShapeGraph 名义依赖，但不会自动编译该类型定义。原循环只按新增 compiled namespace 继续，因此遗漏新发现的名义依赖。现在也按补齐后 compiled deps 指向的待补齐 namespace 继续已有闭包循环；不预编译所有已加载模块，也不让共享测试 runner 更改选择语义。

## 验证

名义解码语义保留在 Calcit :tests；Rust JS host 回归检查干净输出中依赖模块存在、无关 namespace 不输出，并让 Node 实际加载入口。共享 runner 验证全 fixture 与单定义选择，另用报告的 Quatrefoil 精确提交验证，避免依赖预先生成的 JS。

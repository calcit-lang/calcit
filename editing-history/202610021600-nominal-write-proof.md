# 名义字段写入证明与 core 方法来源

## 依据

#1539 的 `state .assoc :count incoming` 最小复现显示两个问题：core Struct 方法表被当作用户无来源 impl 拒绝；恢复方法解析后，已知 Number 字段的 Dynamic 写入没有进入共享 proof。仅增加 fix 规则名无法交付有效迁移建议。

## 实现原则

Struct 的内置方法识别依赖当前真实 core 表共享的字段和值存储身份，不依赖可伪造的名字。保留用户 nominal trait 的候选解析，不把任意同名 `.assoc` 当成 Struct 内置更新。

普通方法调用仍由编译器选择并 lowering 为底层调用。字段/值提取与现有 Struct 更新检查共享，审计通过已有方向性参数证明验证 value，不建立第二套兼容关系。前后缀方法与底层写入都应用相同证明，保留真实实参序号和 value 位置。Dynamic 字段本身仍是开放边界。

`nominal-write-proof-v1` 复用现有 compiler-review adapter、诊断编号、EDN/JSON envelope 与无写回策略。它不改变默认全面收紧的发布时间，也不自动改字段声明、插入强转或把 Struct 改成 Map。先前 helper 错误仍阻断审计，不能伪装成空建议。

## 验证与剩余范围

CLI 回归使用 Calcit 定义附带的测试实际执行 `.assoc`，再检查 Number 正例、Dynamic 前后缀反例、矛盾值、缺失字段、准确源码位置、EDN 与 apply 不写回。跨后端回放、完整回归与消费者验证仍需在 PR 合并前完成。

本步不宣称完整 strict workflow、跨 helper 最早擦除链、自动 metadata synthesis 或 Ref alias 不变量已经交付；保持 #1539 的原验收范围。

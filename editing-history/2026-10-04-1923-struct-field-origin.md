# 按声明来源解析 Struct 字段

## 依据

Diary 在正式 0.29.0-alpha.2 上构造 Database 时，字段中的本地 User 与函数返回的
app.schema/User 被当成不同合同。独立两 namespace 的 Map 字段复现同样失败；
只将字段写成全限定名便通过。对应 #1757，属于 #1529 的真实迁移证据。

## 决策

复用现有 nominal definition_ref 和 namespace resolver，让字段读取、更新、构造和
泛型参数推导使用同一字段合同。先解析声明中的名字，再代入调用者的泛型参数，
不以调用者 namespace 或短名字相等猜测 nominal 身份，不改变底层运行时布局。
没有新增诊断、CLI 入口、Dynamic fallback、unsafe 或全局类型发现。

## 验证边界

现有 def-value-schema fixture 新增五个 definition tests：跨 namespace Map、Optional、
泛型字段、调用者泛型类型来源和不可变更新。现有 known-assertion runner 回放同一 Calcit 表达式到 JS，
并拒绝同短名异来源及错误 primitive 字段，检查拒绝时源码不变、不生成应用 JS。
native/JS 是本次直接覆盖目标；WASM 全量门禁由既有 CI 执行，不宣称新增 Map WASM 支持。
Diary 的存储/凭据与剩余 Router Optional 空 Map、旧依赖问题分开验收，不修改业务断言。

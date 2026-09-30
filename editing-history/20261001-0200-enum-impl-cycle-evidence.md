# Enum impl 循环中的 nominal 证据恢复

## 问题与决策

#1574 来自 form.calcit 的真实迁移：普通 check-only 通过，strict definition graph
却将 `%:: FormPlugin ...` 的返回类型退化为 `:enum`，要求消费者补冗余 nominal 断言。
最小复现需要 trait 方法体内的 `match self`；仅声明 self schema 或常量方法体不能复现。

复用现有 nominal 源码 resolver，沿 Symbol/Import 和 `def`/`impl-traits` 的数据定义路径
恢复 Enum 形状。Symbol 与 Import 共用已有循环保护。构造器先保留已就绪的完整 runtime
metadata；只有无法取得 Enum metadata 时恢复已声明的源码形状，不执行未完成的 impl，
不臆造完整方法表，不扩大 Dynamic，不降低 strict 门禁。

## 验证边界

- fixture 用 Calcit CLI 创建及修改，`:tests` 验证构造后正常方法调用。
- native/JS 复用 fixture 语义；Rust 测试只检查普通与 graph CLI 一致性及错误 payload 拒绝。
- form.calcit 临时副本删除冗余断言，构造器闭包的 8 个定义通过。
- 原安装 Respo 与最新 ToString bound 不匹配，不能宣称其全部通过；临时副本使用已迁移的
  Respo source（unreleased）后 192 个定义通过。原消费者 Snapshot 与依赖均未修改。
- 全量 cargo、clippy、check-all 与真实消费者 JS 回归记录在 PR；不得将运行中的验证写成成功。

本项不新增 nominal 类型身份、alias 目录或 CLI，不改变公开 schema、命名或失败语义。

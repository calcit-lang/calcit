# 源码函数别名调用漏检

## 问题

0.28.0-alpha.1 的 std 消费者验证发现 calcit#1579：直接调用 String→String 函数拒绝 Number，但 `def old-name module/new-name` 别名会通过严格检查。query 显示精确 Fn 不等于调用已受约束。不能靠 std wrapper、native call 或 Dynamic 放宽隐藏它。

## 修复

扩展已有 callable lookup：仅沿编译后直接 Import 引用查找实际函数，使用局部 visited 集合阻止循环，再进入原有正常 Fn 调用分支。保留原始调用 AST，复用 arity、参数、泛型、callback 注入、nominal rewrite 与严格检查；不新建平行 analyzer，不 eval 任意 lazy body，也不从期望 schema 伪造 Fn。

五个 Calcit definition `:tests` 覆盖直接/链式/局部值/callback 传递、泛型、inline callback 上下文、optional 与 rest。Node 只负责严格失败退出码/诊断以及把相同 test AST 通过现有 CLI 写入临时入口并执行真实 JS，不重复实现语义断言。九个负例必须在 preprocessing 中失败。

全 targets clippy 暴露已有 number.rs 的 test module 位置告警；仅将原测试模块移至文件末尾，不修改数值行为或测试断言。

## 兼容与验收

不改 public API、native ABI、JS 输出加载方式，不删除旧名字，也不新增 WASM 一等函数能力。std draft PR #75 仍使用已发布 alpha.1；核心修复通过 review/CI 并发布下一候选版本后才能更新依赖与复验，不用本地源码 CLI 冒充正式消费者验收。

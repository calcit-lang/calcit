# 具体返回值的独立证明导航

为已有 fix 入口提供独立的 `concrete-return-proof-v1` 选择，复用 assertion audit 已实现的
共同 compiler proof、源码重处理与 compiled-state checkpoint。不添加类型关系或诊断编号；
使用既有 `E_FN_RETURN_UNPROVEN`，避免 wrapper 的声明成为 producer 实现的循环证明。

报告保持 source owner/path、fingerprint、origin 与非零退出。只允许所选项目 scope 内的导航；
在 scope 外发现 producer 时要求显式选择 owner。未解决的断言、权限、矛盾返回等其他错误
仍阻断审计，不因为选择返回规则而隐藏。

不从预期返回类型猜写 schema。缺少实现证据只能 review；只有独立证明的缺省 metadata
才能交给已有 synthesis 规则及其调用点/原子事务门禁。本阶段不翻转默认检查、不加入自动 preset，
也不宣称 callable、nominal-write 或完整 strict workflow 已交付。

共享语言行为继续由现有 return-boundary Calcit tests 与 native/JS/WASM runner 验证；
新增 Rust CLI 回归只检查导航、scope、EDN/JSON 报告与 apply 不修改 source 的工具边界。

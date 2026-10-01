# 显式 trait 调用的重复实现边界

## 现状与选择

最新 main 的严格预处理已拒绝同一 origin 的多个候选，但 native runtime 仅在严格模式检测，JS runtime 在命中第一个候选后立即返回。隔离 Snapshot 实际验证了严格入口拒绝、兼容 native 与生成 JS 均采用 nominal 最后一个候选；不把这一复现误写成严格源码漏检。

显式选择 trait 只能消除不同来源间的二义性，不能为同一来源选择实现。统一 native/JS 的 `&trait-call`：先收集同来源候选，多个候选均报现有重复实现错误，包括兼容路径。删除 native guard 的模式条件；JS 复用现有 lookup/identity，不引入 registry 或编译产物模式开关。普通兼容 dot dispatch 不在此次行为修改范围内。

## 验证边界

用户可观察的 Struct/Enum 重复实现失败写在既有定义的 `:tests`。既有严格/兼容策略测试重放这些表达式到兼容 native 和实际 JS，以触达 runtime；trait 回归运行器验证严格 native/JS/WASM/WASI 入口仍必须在预处理拒绝。已有正常 trait、generic bound 与 qualified-origin 测试保持运行。

全量集成验证发现普通脚本的兼容模式扫描门禁：因此兼容 native/实际 JS 回放由既有 `check-strict-default.mjs` 管理，普通 trait runner 仅做严格逐项拒绝与正常程序回放。没有扩大 allowlist 或隐藏开关。

JS host 可以直接构造 impl 表，无法由稳定 Calcit 严格接口构造的 builtin 重复、同名不同 identity、旧 receiver 配合新 identity 与无候选边界在原有 runtime identity 脚本验证；拒绝不得调用任何候选。

本次没有新增 trait default 方法语法、reload 协议或新的检查入口；相关验收须以现有支持范围继续核对，不能把该修复宣称为 #1532 的全部验收完成。

补充验收：新增同一 trait 工厂重新求值的 Calcit `:tests`，native 与实际 JS 均保持旧身份可调用、拒绝新身份选择旧 impl。源码审阅显示 `defaults` 仅由 Rust 内部测试注入，公开 constructor 固定为 None；现有严格 runner 在 native/JS/WASM/WASI 上验证 `(method type default-body)` 在 macro 层拒绝，并在用户文档说明未提供默认方法体。原有 Rust trait-schema-owner 缓存依赖与同 source-ref 的不同 runtime identity 回归继续保留，不把工厂重求值测试冒充完整 watch 端到端验证。

首轮远端 CI 在 compatibility JS import 报 `ERR_MODULE_NOT_FOUND`：严格策略测试原先不执行 JS，工作流把 runtime 编译/本地包链接放在后续 try-js，开发环境预先存在链接掩盖了顺序问题。测试与发布工作流都将原有 runtime 准备前移到严格策略测试，后续 try-js 复用产物，避免重复链接；不跳过真实 JS 回放、不重跑旧 HEAD 冒充修复。

# 已解析 Struct 构造的字段完整性

## 问题与决策

Alerts prompt 的 DomProps 迁移暴露出遗漏字段的 `%{}` 构造可以通过默认严格检查和 JS codegen，却在实际生成的 JS 执行时报错。最小复现和验收由 #1776 跟踪；这属于 #1553 展开后校验的共同语义，不增加模块特例或新的 analyzer。

在既有 `check_struct_construction_fields` 中，解析出 nominal prototype 后同时验证固定参数的字段数量、重复或未知字面量字段名，并保留字段 payload 证明。复用 `W_FN_ARG_TYPE_MISMATCH` 和已有调用位置选择。包含参数展开时不根据未展开的 AST 数量判断完整性；真正动态的 prototype 继续由运行时验证。不增加默认值、不扩大 Dynamic、不改写业务源码。

Review 提醒非 Tag key 不能被当作字段覆盖证明，但 native 原有 String 和引用 Symbol 名称是合法的，因此不直接禁止非 Tag。同一字面量名称 resolver 用于结构和 payload 验证，拒绝已知重复/未知名称及 Number/Bool/nil/Unit key。保留运行时字段名与最终运行时校验；必要的参数数量检查不是向类型系统输出完整性证明。

核对运行时发现 String/Symbol 分支没有沿用 Tag 分支的重复字段守卫；动态 String 名称重复时可能留下未初始化字段。因此补齐同一个 `seen_positions` 守卫，合法动态名称继续可用，重复名称统一失败，并用附带 Calcit 测试验证失败而不是返回残缺 nominal 值。

## 验证

Calcit definition `:tests` 保存完整构造、重排、局部别名和动态 prototype 的正例，现有 Struct JS 回放执行同一函数。现有已知断言运行器加入缺字段、重复字段、未知字段、局部别名、跨 namespace prototype/别名和错误 payload，在 native、check-only、JS、WASM/WASI 及其 check-only 模式验证拒绝、源码定位和无应用产物。

保留原有字段求值/失败顺序、命名构造、Option 和 native/JS/WASM 回归；本次不调整 raw primitive 的严格入口限制，也不以检查通过代替真正执行验证。

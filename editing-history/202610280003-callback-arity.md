# 已知回调的参数数量属于调用合同

#2430 从 Cyan Red 的事件回调迁移中发现：已声明 `Fn(Number) -> Number` 的参数被调用为
`callback 7 8`，全定义检查、完整 strict 与 JS codegen 仍接受，直到实际调用才失败。
0.31.1 的独立无模块复现仍能重现此差异。另一个同类入口是已知函数的字面量 List 展开：
空展开后多出的实参也必须计入数量，不能只依赖后端拒绝无效调用。

## 决策

在已有 callable 检查中复用 `call_arity()` 的最小/最大范围，先校验数量，再校验实参类型。
零参数签名同样检查；参数中的 Fn、词法别名、捕获和计算出的回调都沿用这一关系。
字面量展开复用现有展开分析，只计算静态可知的数量，不改写或执行实参。
诊断复用 `W_LOCAL_FN_ARG_TYPE_MISMATCH` / `W_FN_ARG_TYPE_MISMATCH`，计算出的回调沿用
已有来源定位工具保留调用定义与路径，不新增诊断编号、checker、规则或 preset。

全量 Cargo 的原 `test-arguments` 揭示，局部函数由 hint 推导签名时丢失了源码 `?` 的数量信息。
修复在推导时保留现有参数形状；`Optional` 值类型自身不代表参数可省略。原 fixture 与断言保留，
旧 `Optional` 的退场继续由 #2233 管理，本次不借参数校验提前删除。

## 兼容边界

该改动新增运行前拒绝，归属 0.32.0，不回移到 0.31.1。合法末尾 Option 省略、显式 Option、
rest 参数及未知长度展开的既有证明/运行时边界不变。裸 Fn 的保存或传递不获得猜测的零参数签名。
不扩大 Dynamic、增加 unsafe 或修改用户程序来规避错误。

## 验证

正常语义放在 `test-fn-inference` 的两个 definition `:tests`，覆盖固定/零参数、别名捕获、
Option/rest、求值顺序与次数、抛错后的短路和裸 Fn 传递，通过现有运行器在 native/JS 回放。
现有 `check-known-assertion.mjs` 增加独立分片，验证 11 类非法调用在默认 proof audit 的
全定义检查、完整 strict、native/JS/WASM/WASI 入口被拒绝，保留 source path、无执行标记或可执行产物。
现有 spread 脚本改为验证更早的源层拒绝，不删除原来的非法调用。

执行完整 Cargo、Clippy、格式检查和原 CI `check-all`，以及真实 Cyan Red / Axis 的完整 strict、
定义测试及 JS 构建。真实项目只用临时副本及已安装发布依赖，候选 CLI 明确为未发布源码，
不把本次验证写成新版本正式配对或生产部署。最终结果记录在关联 PR 中。

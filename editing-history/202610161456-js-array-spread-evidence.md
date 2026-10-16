# JS 数组展开的返回证据

## 原因与范围

Lagopus 的 `make-empty-js-arrays` 用 `js-array & (map ...)` 创建多个独立的宿主数组。
普通检查与生成 JS 正常，返回证明却把整个表达式视为 unknown。固定参数的
`js-array` 已有独立 `JsObject` 证据，非字面量 spread 路径只识别 Fn 签名，未复用此事实。

在现有 CallSpread 返回推断中，为真实的 `js-array` intrinsic 描述其既有的
零个固定参数、开放 rest 参数与 `JsObject` 结果；随后复用原 typed-rest 投影及
CallTypeProof。局部遮蔽仍走局部 callee 的合同，不按名称赋予构造器身份。

## 语义与边界

不改变 AST、调用参数、codegen 或运行时；不增加 API、诊断、fix rule、类型变体、
unsafe 或默认值。不从期望返回类型反推实现，也不将不透明宿主调用认作构造器。
末尾 spread 仍须证明为 List；开放元素不提供额外的具体类型或宿主 shape。
其他 spread 形状保留现有的保守返回行为，不借本次修复扩大证明范围。

## 验证设计

- 用户可观察的空数组、独立子数组、单个子数组修改、nil/false/0/空字符串和求值顺序
  放入现有 fixture 的 definition `:tests`，由现有运行器执行生成 JS。
- 现有断言集成测试检查返回 proof、query type-at 与完整 strict；保留固定字面量、
  开放元素、显式 leading 参数、core wrapper 与结果 alias 正例。
- 反例覆盖错误返回、伪造元素类型、不透明 JS 调用、非 List/未知 spread、局部遮蔽、
  错误 map 回调和缺少 js-ffi feature；拒绝时不修改 Snapshot。
- 用正式 0.30.0 与本地源码候选在 Lagopus 的原定义做只读前后对照，不更新下游源码、
  依赖或运行 GPU。该候选验证不替代正式发版后的全项目验收。

宿主数组本身只在 JS 运行，不宣称 native/WASM 同等支持；通用 rest 关系仍由原跨后端
回归检查。新增测试不需要 Rust 重复语义测试或独立 checker。

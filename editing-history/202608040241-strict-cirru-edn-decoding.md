# Strict Cirru EDN decoding

## 背景

`parse-cirru-edn` 面向动态 EDN。它的 options map 只能按名称恢复部分 record/enum 身份，不能证明容器元素、字段、enum payload、泛型实参和约束符合业务类型；字段集合不一致时，Native 旧路径还可能触发 panic。

## 本次决策与实现

- 先形成 `RFCs/08-04-strict-cirru-edn-decoding-rfc.md`，把动态解析与严格类型边界分开。
- 新增 language syntax `parse-cirru-edn-as text TypeExpr`；预处理阶段要求目标类型闭合且可解码，静态推断结果直接采用目标类型。
- 编译器把类型表达式派生为不含 `Dynamic` fallback 的 `EdnDecoderGraph`。Native 从 `cirru_edn::Edn` 深度验证并构造 Calcit 值，JS codegen 输出同构 graph 交给 runtime 执行。
- struct/enum 解码检查精确名称、字段集合、variant、payload arity 和深层字段类型，并保留真实 nominal declaration；泛型应用检查参数数量和 `:where` 约束。
- Native/JS 错误都携带结构路径，例如 `$[0].age`。
- JS nominal struct 的字段顺序由 interned tag 决定，可能不同于 Rust 声明解析使用的词法顺序；JS 严格解码先按 graph 校验，再按运行时 `nominal.fields` 重排 values。
- 旧动态 record options 字段不匹配时改为返回 loose record，避免 `unreachable!` panic；JS 保持相同行为。

## 边界与后续

- Phase 1 支持有限闭合类型图，不承诺递归 struct/enum。现有自递归名义类型在预处理构造路径会栈溢出，已记录为 GitHub Issue #295；修复 cycle-safe 类型解析后再开放递归 decoder。
- 一等、不可伪造的 `EdnDecoder<T>` dictionary 延后到 Phase 2。当前 graph 只作为编译器内部闭合程序，避免重新暴露动态 schema 逃生口。
- Timegrass 的 snapshot 临时副本已使用当前 `cr --check-only` 回归通过，未写入业务仓库。

## 验证要点

- Rust 单测覆盖深层 struct/enum、错误路径、Dynamic/缺泛型拒绝、where-bound，以及旧动态 record mismatch 非 panic。
- `calcit/test-edn.cirru` 同时覆盖 Native 与 JS：本地及 imported nominal type、嵌套容器、泛型 struct、enum、运行时失败和 top-level decode 顺序。
- 文档示例由 `cr docs check-md` 验证。

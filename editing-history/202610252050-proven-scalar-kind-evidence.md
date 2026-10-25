# WASM 在标量擦除前保留实际 kind

## 问题与决定

已声明为 Bool 的参数交给 `number?`，native／JS 返回 false，但 WASM／WASI 的 `type-of` 堆指针猜测把非指针的 0／1 一律视为 Number。类型检查成功不能代替实际后端语义。

在 `type-of` 的同一个 lowering 入口复用已有推断结果：Number（含数值 refinement）、Bool、Nil、Unit 的确切 kind 优先于物理位值。仍执行一次原实参，再丢弃其结果并生成类型 tag；类型谓词继续调用该入口，不再单独维护 Number 正向特例。只有具体标量证据才能决定结果，不能把“未证明为 Number”当作“已证明不是 Number”。

对 `number?` 的特化需求判定复用同一个具体标量分类。已知 Bool 的递归函数不应仅因存在一个无关泛型参数就触发不支持的递归特化；操作数中的其他敏感表达式仍需继续检查。

## 验证与边界

正常语言语义写在 `calcit.core/number?` 的定义测试中，覆盖 Bool 的两个值、Nil／Unit、Number 0／1／负零／NaN、数值 refinement、高阶回调及求值顺序。现有 core 运行器和 CI 在 native／JS／WASM／真实 WASI 回放这些测试，不新增 runner、检查入口或排除。

已有 WASM fixture 补充 Bool／无关泛型递归与真实参数失败路径。宿主断言只覆盖实际 trap 和单次日志；既有 Number 与存活堆地址同值的断言保留。

Unit 作为既有语言 kind，和 Number／Bool／Nil 一样预登记为内置类型 tag。扩展原有内部 tag 登记测试，验证没有源码 tag 字面量时也能取得这些 tag；不新增另一套 Rust 语义测试。

不改 ABI、公开 API、严格检查接受范围、Dynamic 或 unsafe 合同。开放与可空标量继续保留原有边界，不声称其运行时歧义已解决；也不借本修改宣称其他方法结果或显式内部 trait-call 的证据问题已经完成。

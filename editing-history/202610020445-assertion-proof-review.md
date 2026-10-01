# 显式断言证明审计

## 目标与证据

断言更新局部类型之后再查询输入，会把断言自身制造的标签误当作独立证明。新增显式审计直接复用
compiler 的 TypeProof，在局部类型更新之前拒绝缺证据的转换，普通编译继续保留既有开放边界迁移策略。
工具复用原 unsafe 边界导航模块，统一定位、指纹、编译栈、只读建议和错误报告，没有另建类型关系或顶层命令。

定向回归包含开放输入、已知矛盾、嵌套断言、未知 callable、未绑定泛型、虚假 producer 返回声明，
以及 Number identity、T identity 和 Dynamic identity 正例。正常语言值用 definition `:tests` 验证；
Rust 只负责低层审计作用域恢复、CLI envelope 与原始 bytes 不变等边界。

## 共同证明与 producer

共同关系允许从实际类型为预期泛型收集绑定，不允许反向把未知实际 T 绑定成具体 Number。
同一个 T 的恒等关系和显式 Dynamic 目标仍合法。此项修正复用原关系，未把 Unknown 或泛型改名成 Dynamic。

同次审计中，函数体缺少具体返回证据时先报告 producer，避免后续断言借用其声明自证。
新 `E_ASSERT_TYPE_UNPROVEN` 区分缺证据与既有 `E_ASSERT_TYPE_MISMATCH` 矛盾；
`E_FN_RETURN_UNPROVEN` 区分缺少实现证明与原返回类型矛盾 warning。既有矛盾门禁不降级。

## 边界与验证

规则只通过显式 `fix --rule assert-type-proof-v1` 开启，不加入默认 preset 或 strict workflow。
所有建议保持 review-required、空 replacement 和零写入；不自动选择业务 decoder 或授予 FFI 权限。
同次审计遇到依赖、core macro 或所选 scope 外的错误仍失败，不添加 namespace 豁免。
每个定义只报告首个错误，修复后应重跑；attached tests 与 examples 不作为代码扫描目标。

定向 CLI 与作用域回归之后仍须执行完整 Rust、Clippy、fmt、JS/Agent/backend 集成与外部项目回归。
记录中的设计和测试目标不是全量验证已完成或版本已发布的声明。

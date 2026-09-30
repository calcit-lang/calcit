# `turn-string` 的公开类型边界

此前 `turn-string` 同时是 reader 识别的 primitive 和公开兼容入口。公开 Fn schema 写成任意 `T -> String`，底层 proc 签名为 `Dynamic -> String`；因此只为 Snapshot schema 增加 `ToString` bound 不会影响严格预处理，List 会到运行时才失败。

本次将 primitive 拼写移到 `&turn-string`，内建 Number、String、Nil/Bool/Tag/Symbol 的 `ToString` 实现继续调用它；旧 `turn-string` 改为普通 Calcit 兼容函数，转调已经受 `T: ToString` 约束的 `to-string`。公开调用因此复用现有 trait 推导，不增加名字特判或新分析器。自定义 `ToString` 名义类型由 trait 正常派发；List/Map/Unit/开放 Dynamic 在严格预处理拒绝。内建标量的文本和求值次数不变。

JS codegen 把内部 primitive 映射到既有 `@calcit/procs` 的 `turn_string` export；WASM 与 native 继续按 `CalcitProc::TurnString` lowering。`core-identity-conversion-v1` 的守卫由 reader 内建 proc 识别改为精确源位置的 core wrapper 解析，同时仍限定六类已证明的标量自动迁移。不能据此把自定义实现或开放 Dynamic 批量改写。

验证：Calcit core definition `:tests` 和自定义 Struct `ToString` 定义测试；严格负例覆盖 List/Map/Unit/Dynamic；`check-identity-conversion-fix.mjs` 验证安全/不安全迁移；`check-typed-string-conversions.mjs` 核对 native、生成 JS 与可用 WASM 子集；并运行仓库全量门禁。旧名退场仍受 #1456 的消费者和发布窗口条件约束。

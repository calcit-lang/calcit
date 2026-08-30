# Blocking callback Unit transport / Blocking callback 的 Unit 传输

## 中文

- 修复 blocking C FFI callback 返回 `Unit` 时，host 输出 `&unit` 而 native adapter 按 Cirru EDN 解码失败的契约不一致。
- 仅在 blocking callback 的返回传输边界把 `Unit` 规范化为 EDN `nil`；Calcit 内部类型仍为 `Unit`，异步 terminal 的显式 `&unit` 协议保持不变。
- 新增真实 host buffer 回归，验证状态为成功、payload 可由 Cirru EDN 解码且 buffer 仍按一次性所有权释放。

## English

- Fix the blocking C FFI contract mismatch where a callback returning `Unit` produced `&unit`, while the native adapter decoded the result as Cirru EDN.
- Normalize `Unit` to EDN `nil` only at the blocking callback result transport boundary; the Calcit type remains `Unit`, and the explicit async terminal `&unit` protocol is unchanged.
- Add a real host-buffer regression covering successful status, Cirru EDN decoding, and exactly-once buffer ownership release.

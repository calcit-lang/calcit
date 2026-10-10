# JS 解码与 native FFI 失败边界

关联 #1935、#1457。正式 alpha.28 的共享 Calcit 取消测试在 native 通过，但 JS 的六项排除仍失败：三个是标量 decoder 错误归属，三个是取消操作没有实际失败。

## 决策

复用同一个 typed decoder，通过失败回调保留调用入口和完整字段路径；默认仍属于 `parse-cirru-edn-as`，runtime map 的标量分支传入 `decode-map-as` 的失败回调。Nil/Unit 的错误名称与 native 的 `nil` / `&unit` 一致，不改变接受的值、深度限制、shape ABI 或类型证明。

仅将 native FFI 的 cancel/resolve/reject 三个 JS stub 改为抛出包含具体操作名的 capability 错误。它们仍可被生成代码正常导入，但调用不能伪装成 Unit 成功；不增加 JS native FFI 实现，不改变其他历史 stub。异常由调用者报告，不再先打印警告。

## 验证与兼容

保留原取消/Unit 测试断言，通过后删除六项 JS 排除。通过正式 CLI 的 transaction dry-run 与 scoped revision 添加两项 response 方法失败测试和一项含 Nil、Number、嵌套 List 路径的解码测试。所有程序、签名和原有测试保持不变；Snapshot serializer 的布局变化另以解析后的结构对比确认。

复用现有跨后端运行器和 runtime identity/parse 检查。新增测试中的 `try` 在 WASM/WASI 仍 unsupported，按实际编译失败登记排除，不借此新增宿主能力或降低断言。宿主脚本只验证 operation-specific 错误和零 console 警告；语言语义留在 Calcit `:tests`。

用户可见变化是 JS 原先静默成功的 native FFI 操作现在会失败，以及 decoder 错误归属和 Nil/Unit 拼写修正。业务不应依赖不支持的取消或响应操作成功。此变更不作为任何后端新获 exactly-once 能力的证据。

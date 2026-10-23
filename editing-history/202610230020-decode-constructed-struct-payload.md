# 已构造 Struct 的受检解码

Issue #2154 区分了 JS nominal enum 身份被重建与已构造 Struct payload 不受 decoder 支持两种问题。前者已由 #2284 处理；本次为后一种情况补齐既有 `decode-map-as` 的运行时边界，不增加项目专用迁移规则或类型检查入口。

当解码图需要一个 Struct、输入已经是 Calcit Struct 时，native 与 JS decoder 只接受匹配的 nominal 定义，并继续检查字段布局、字段数量与每个字段的递归目标类型。全部字段无需转换时返回原值，以保留不可变值身份；若嵌套字段需要 decoder 现有的转换行为，则重建 Struct。Map 输入路径保持原样。

此运行时能力不改变静态已知 Struct 根节点的防双重解码诊断，也不放行不同定义、字段布局不符或 payload 类型错误。它用于动态/开放值中嵌套的已构造 Struct，调用方仍应在可用时保留静态 nominal 类型。

验证包括 native/JS `decode-map-as` 定义测试、Rust nominal/layout/payload 单测、TypeScript 编译、Rust Clippy 与 library 测试。WASM 的 typed decoder 仍不支持，未改变对应后端合同。

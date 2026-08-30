# Strict FFI lowering validation / 严格 FFI lowering 校验

- Validate native base symbols, invocation modes, versioned transports, and
  published invoke/transport pairs while exporting Interface IR v1.
- Reject unknown backends and incompatible native/JS targets with stable,
  path-specific diagnostics before generation.
- Keep IR v1 schema stable and document its direction invariant: Calcit
  imports callable bindings from the selected host backend.
- Preserve the Phase 0 std/regex/wss export counts while making legacy
  incomplete metadata explicitly unsupported.

- 在 Interface IR v1 导出阶段校验 native base symbol、invoke、版本化 transport
  及已发布的 invoke/transport 组合。
- 对未知 backend 与不兼容 native/JS target 输出稳定、精确 path 的诊断。
- 保持 IR v1 schema 不变，并明确其 direction 不变量：Calcit 从选定 host
  backend import callable binding。
- 保持 Phase 0 std/regex/wss 导出数量，同时让旧的不完整 metadata 明确变为
  unsupported。

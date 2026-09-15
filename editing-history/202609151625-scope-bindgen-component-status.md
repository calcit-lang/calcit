# 限定 bindgen Component 状态

- 将 README 中笼统的“runnable Component packaging 仍在推进”拆成已完成与后续范围。
- 明确 0.14.20 的同步类型子集已经可以生成 `component.wasm`，并通过 Wasmtime 与 jco/Node 验证。
- 保留 Struct/Enum、resource、async 等更广 Component packaging 为后续工作，避免把局部闭环描述成全部完成。

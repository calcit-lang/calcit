# 准备 Component core preview 发版

- 把 0.14.20 定位为 Number/String 同步 Component contract 与 Canonical ABI adapter 的 core preview，不宣称已经生成 runnable Component。
- 明确 Component WIT generation、component packaging、Wasmtime/jco 与复合类型仍属于 0.15.1 后续工作；native Interface IR 的 WIT generation 已由 `calcit-bindgen` 提供。
- 记录宽度明确的 WIT 数值类型不能从当前统一 `Number` 中猜测，应先形成可推导的 source-level 表示。
- 修正 `calcit-bindgen` “experimental production” 的矛盾状态描述，并更新升级手册中的固定版本示例。
- `cr-wasm` 仍承担内部自举回归；停止发布或删除应在独立兼容性任务中处理，不阻塞本次小版本。

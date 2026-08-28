# 消费共享 raw ABI / Consume the shared raw ABI

## 中文

- Calcit runtime 直接依赖 `calcit_native_ffi 0.1.2` 的 no-default-features raw contract。
- 删除 host 内重复的 buffer/async/blocking/resource version、symbol、function pointer、C-layout descriptor 与数值常量定义。
- 保留类型安全 handle/event enum、event queue、task/resource registry、lease、dylib cache 与 callback lifecycle 在 Calcit host。

## English

- Made the Calcit runtime consume the no-default-features raw contract from `calcit_native_ffi 0.1.2` directly.
- Removed host-local copies of buffer/async/blocking/resource versions, symbols, function pointers, C-layout descriptors, and numeric constants.
- Kept typed handle/event enums, the event queue, task/resource registries, leases, the dylib cache, and callback lifecycle in the Calcit host.

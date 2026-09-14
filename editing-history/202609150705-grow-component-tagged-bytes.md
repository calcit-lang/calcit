# 修复 Component tagged bytes 的内存增长

- Component boundary 的 String 与 Buffer lift 统一通过 `cabi_realloc` 分配 tagged bytes，避免大值或重复调用越过当前线性内存。
- 分配前检查 padding、payload 和 header 大小的 32-bit 溢出；native boundary 保持原有 bump allocator 行为，同时补齐相同的溢出保护。
- Node 集成测试使用超过初始 memory 大小的 Buffer 连续 round-trip 两次，覆盖真实 `memory.grow` 路径并验证增长后数据完整性。
- 该修复回应 PR review，未增加新的语言规则、类型分支或 CLI 入口。

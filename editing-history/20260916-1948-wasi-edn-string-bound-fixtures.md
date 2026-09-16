# WASI Cirru EDN 字符串边界回归

- 增加超过 64 KiB 的源字符串 fixture，直接覆盖 `__rt_edn_string` 的输入长度保护。
- 增加小于 64 KiB、但转义后超过 64 KiB 的换行字符串 fixture，覆盖编码后输出长度保护。
- 继续保留集合拼接超限 fixture，三个路径都由真实 Wasmtime 执行并断言 trap。

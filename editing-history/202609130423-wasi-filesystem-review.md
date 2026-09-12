# WASI 文件边界 review 修复

- 为 `&fs-read-text` 与 `&fs-write-text` 补齐 `:file` effect 标签，使 effect graph 能识别内部文件边界。
- `fd_filestat_get` 的 64 字节输出改写入线性内存顶部的保留 scratch 区，避免覆盖从 `HEAP_BASE` 开始的静态字符串。
- 假宿主精确断言 filestat 指针位于 scratch 区，覆盖这条底层内存布局契约。

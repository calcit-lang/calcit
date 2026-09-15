# 加固 Component List 内存边界

- Canonical List lift 在读取元素前验证指针对齐、长度乘法溢出和完整内存范围。
- List lower 在读取内部 count 与元素前验证精确指针、8 字节对齐、整数 count 和完整内部区域，并在写入前验证 `(ptr,len)` return area。
- Node 集成回归新增未对齐 Number List、长度溢出、未对齐 List pair、未对齐嵌套 List 与越界嵌套 List 输入，确保边界统一 trap。

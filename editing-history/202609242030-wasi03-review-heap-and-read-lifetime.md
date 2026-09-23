# WASI 0.3 文件审查：堆增长与读取局部变量生命周期

## 缘由

PR #1316 的 CodeRabbit 审查指出两个内存安全问题：`FsPath.read-text` 的 WASM lowering 只在成功读取时更新 `data` 局部变量，而错误分支会尝试释放非零 `data`；固定大小 bump 分配在大块动态分配扩容后仍可能越过当前线性内存页数。第二项已通过旧代码上的真实 Wasmtime 49 复现：读取 4 MiB 文件，再在同一命令中反复构造小型 `Result`，在内存边界 `0x810000` 越界 trap，而不是正常完成。

## 修复与边界

- 每次调用私有 `__rt_wasi_component_read_bytes` 前把 `data` 设为零；失败路径不再依赖 WASM 函数入口时的一次性零初始化。成功/无效 UTF-8 路径仍按原契约释放数据、输出记录和文件资源。
- 抽出共用的内存页覆盖检查，在动态和固定大小 bump 分配写入 header 之前执行；增长失败明确 trap，不允许静默写出线性内存。固定分配同时检查结束地址相对起点的溢出。
- 不增加新的 Calcit 表层 API、Dynamic 逃生口或检测规则。两个回归入口仍通过公开的 `calcit wasi --boundary component` 编译，在真实 WASI 0.3 host 上运行。

## 验证

`main-read-loop!` 在一个 Calcit `each` 中先读有效文件、再连续读缺失文件，要求返回 `Result.err` 并完成；`main-growth-loop!` 先读取 4 MiB，再构造 5000 个小型 `Result`，要求无内存越界并输出完成标记。前者检查用户可见错误语义，后者覆盖 WASM heap/ABI 的低层不变量。两者由同一 Rust/Wasmtime 集成测试启动，以真实 Component 验证；原有 definition `:tests` 继续覆盖文件方法的正常与失败语义。

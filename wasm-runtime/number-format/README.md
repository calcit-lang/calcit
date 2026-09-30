# WASM Number 格式化 helper

这个无宿主函数的源码用于再生 `src/codegen/emit_wasm/number-format.wasm`。它调用固定版本的 Ryū 产生最短十进制表示，并在模块内部把指数展开为 Calcit 的普通十进制文本。输出缓冲区由 Calcit 运行时提供 400 字节，负数最小次正规值的文本长 327 字节。Calcit codegen 把该模块的函数和只读数据嵌入最终 core WASM/WASI 模块；发布的 `calcit` 不调用外部编译器或 `wasm-merge`。

再生需要 Rust `wasm32-unknown-unknown` target，执行 `bash scripts/build-wasm-number-format.sh`。脚本固定内存布局：辅助栈位于低地址，只读表从 8192 开始，Calcit 的字符串池和堆从 16384 开始。修改依赖、编译参数或源码后必须重新生成并运行 WASM、WASI 字节回归。Ryū 1.0.23 的源码采用 Apache-2.0 或 BSL-1.0 双许可证；依赖版本由此目录的 `Cargo.toml`/`Cargo.lock` 固定。

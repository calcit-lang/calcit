# JS FFI 文件改为表达式嵌入

修正 0.22 预览实现：原方案复制 ESM 文件及 helper 资产，并允许 snippet 之间用相对 import 连接。Calcit 的模块转写会改变输出文件位置，该假设不成立。

- `:inline` 和 `:file` 现在都提供求值为函数的单个表达式；文件源码在构建时嵌入生成的 Calcit namespace。
- 移除 `:export`/`:assets`，拒绝源码中的顶层 `import`/`export` 声明；不同定义引用同一个文件时分别求值。
- `:modules` 显式声明 `node:` 内置模块或裸 npm 包 specifier；生成文件负责 import 并向表达式传入别名。相对/绝对/URL 模块路径不支持。
- 生成代码保留 namespace/definition 定位注释；精确 source map 仍是后续工作。
- 跨模块 demo 验证产物搬移后可运行，且无需复制 `.ffi` 资产；Rust 测试覆盖路径边界与旧元数据拒绝。

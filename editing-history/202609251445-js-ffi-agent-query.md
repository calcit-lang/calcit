# JS FFI 的 Agent 查询来源

模块内 JS 表达式合并后，`query def` 原先只展示通用 `:ffi` 元数据；Agent 需要手动解释 EDN，仍不容易确定依赖模块根目录与可编辑 JS 文件。`query context` 也没有对应来源提示。

本次沿用现有 `query def/context`，增加归一化的 `js-ffi` 来源字段和 human Markdown 区块，不创建新的顶层命令。通过模块加载器已有的 namespace source 映射追溯依赖的 owner；路径只是本机定位线索，不写入 Snapshot，也不作为可移植身份。inline 代码在 `query def` 中使用独立 JavaScript fenced block；`query context` 只给简要来源与下一步查询，避免在有界上下文中复制任意长的 JS。损坏的 JS FFI 元数据仍可查询，并报告解析错误。

验证包括依赖模块的 inline/file 查询、EDN/JSON 输出、Agent interface smoke、Clippy 与 CLI 实际调用。查询只读取元数据，绝不执行 JS；Fn schema 仍是作者声明，不是 JS 实现证明。精确源行映射由 #1362 跟进。

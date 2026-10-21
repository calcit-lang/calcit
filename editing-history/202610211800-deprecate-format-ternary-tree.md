# 弃用 `&format-ternary-tree` 并登记 0.32.0 删除

List 已迁到 FingerVec（#1633），旧 `&format-ternary-tree` 仍以三叉树名称暴露，实际返回 FingerVec 的调试文本。Explain Ternary Tree 迁移时把这段文本当作树结构解析，严格检查和构建都通过，但演示数据已失真（#2193）。维护者确定 0.31.0 只做弃用，0.32.0 删除，不在编译器里恢复旧格式或增加解析兼容层。

沿用 core 现有的弃用机制，而不是新增改名入口：定义加 `:deprecated` 标记，`query def` 的文档说明它是 FingerVec 诊断、文本不是数据合同、0.32.0 删除且没有替代；`analyze weak-types --only deprecated-call` 由同一标记列出调用点。没有另起新名字，因为 0.32.0 计划直接删除这个 helper，新名字只会多一个需要再退场的入口。

运行行为不变。native 照常返回字符串；JS 发射原本就拒绝这个 native-only 原语，报错末尾补充弃用与删除版本；WASM 原本就报告 unsupported proc，支持矩阵不变。proc 失败描述改为“不是 FingerVec 支撑的 native List”，不再提三叉树。

附带 `:tests` 固定删除前仍返回 String。测试的实参是常量，预处理阶段已在 native 求值折叠，因此统一回放在 JS 与 WASM 上也通过，不需要排除项；运行时引用被 JS 发射拒绝的路径继续由 `tests/js_namespace_import_cli.rs` 覆盖。升级手册新增 0.31.0 弃用 / 0.32.0 删除小节，给出按用途的迁移对照；`docs/data/README.md` 的说明同步改写。

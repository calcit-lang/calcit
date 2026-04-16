# WASM: Cross-namespace calls & host imports (pow, sin, cos)

## 概要

为 WASM codegen 增加了跨命名空间函数调用和宿主函数导入两大功能。

## 知识点

### 跨命名空间调用

- `emit_wasm()` 从只处理 `init_ns` 改为遍历 `program_data` 全部命名空间
- `fn_defs` 从 3-tuple `(def, args, body)` 扩展为 4-tuple `(ns, def, args, body)`
- `fn_index` 同时保存 `"ns/def"` 全限定名和 `"def"` 裸名两种键
- `emit_call_expr` 中遇到 `Import` 节点时，先尝试全限定名再尝试裸名
- `collect_all_tags` 更名为 `collect_all_tags_from` 适配 4-tuple

### 宿主函数导入 (Host Imports)

- 新增 `HostImport` 结构体和 `HOST_IMPORTS` 常量数组（pow/sin/cos）
- WASM 模块新增 `ImportSection`，从 `"math"` 模块导入函数
- **关键约束**：导入函数占据函数索引 0..N，用户函数从 N 开始
  - TypeSection: 先写 host import 的类型, 再写 user function 的类型
  - FunctionSection: 只写 user function（import 自动拥有类型）
  - ExportSection / fn_index: 用户函数索引均需加 `num_imports` 偏移
- `emit_host_call()` 辅助函数：按名称查找 HOST_IMPORTS 并发出 Call 指令
- `CalcitProc::Sin/Cos/Pow` 从返回错误改为调用 `emit_host_call`

### 测试

- `test-wasm.cirru` 新增 `test-wasm.helper` 命名空间（含 `add-and-double`）
- `test-wasm.mjs` 新增 `math` 导入对象 + `checkApprox()` 浮点近似断言
- 测试从 31 项增加到 39 项，全部通过

## 注意事项

- WASM 规范要求 Import section 必须在 Function section 之前
- 添加新 host import 时需更新 HOST_IMPORTS 数组，并确认 JS 测试 runner 提供对应实现
- 后续可考虑将 host import 模块名参数化（目前硬编码 `"math"`）

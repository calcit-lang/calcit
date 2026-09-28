# `integer?` 公开入口的第一小批

## 背景与决策

0.27.0 已修复 `round?` 在 native、JS、core WASM 与 WASI Component 上的近零和无穷值差异；其现有契约是判断 Number 有限且恰好没有小数部分，不执行舍入，也不证明安全整数范围。#1481 需要让名字直接表达这个命题。第一小批增加公开 `integer?` 与 Number `.integer?`，并通过现有 `round?` 实现保持同一失败与求值语义；旧入口继续兼容，不在此批删除或换义。

## 类型与迁移边界

新函数公开 `Number -> Bool` schema，新方法由 Number 接收者解析。不能把 Bool 结果当作 Int32/UInt32 refinement，也不能接受 Dynamic 来绕开类型检查。旧 `round?/.round?` 的来源受控 fix 独立实现和验证；在此之前，文档不能宣传自动迁移已存在。附加 `:tests`、`:examples` 与自定义同名函数不按文本批量替换。

## 验证方式

新增 Calcit definition `:tests` 覆盖正负零、普通/极大整数、非零小数、NaN、无穷值、方法调用与参数求值；现有数值谓词共享测试脚本将同一 AST 在 native、生成 JS、core WASM 和 CI 的 WASI Component 路径重放。再核对 Agent 可查询的公开 schema 和 Number 方法契约，运行 Rust、文档与 Agent 接口检查。旧 `round?` 测试不删除。

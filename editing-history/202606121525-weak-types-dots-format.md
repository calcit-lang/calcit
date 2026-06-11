# 修改记录: cr analyze weak-types 路径打印格式优化使用点号 (2026-06-12)

## 知识点与变动树

1. **Weak-type 路径打印格式优化**:
   - 原本 `cr analyze weak-types` 命令输出 AST 节点和声明模式中的位置时会使用大量的方括号 `[` 和 `]`，如 `code[3][1][6]`。
   - 现将其重构并统一为标准点号 (`.`) 路径分隔法（如 `code.3.1.6`），以消除视觉冗余。

2. **具体实现 & 多位置对齐**:
   - `format_cirru_path`: 修改它的实现，将其内部的 `[idx]` 拼接机制替换成了 `.idx`，从而全局对齐了 Cirru 节点遍历的 dot 符号写法。
   - `scan_schema_dynamic_annotation`: 内部处理 Fn 类型的索引时，把 `{path}.args[{idx}]` 和 `{path}.type-arg[{idx}]` 修改为点号连接的 `{path}.args.{idx}`、`{path}.type-arg.{idx}`，并同步修改了 `schema.args.{idx}` 语法签名位置。

3. **测试与质量验证**:
   - 执行 `cargo test --bin cr` 通过了全部 85 项内置和回归测试（无测试报错）。
   - 实地重新构建 `cargo build --bin cr` 并对大规模项目 `termina/msg-buffer` 完美执行 `analyze weak-types` 分析，其输出完美显示为高清晰度的 `code.6.1.3` 及 `schema.args.0.item.item.value` 点号格式。

# WASI typed EDN quoted String

## 设计

WASM typed parser 继续由闭合 `DataShapeGraph` 驱动。String 节点在已有 `|text` 之外，直接解码 formatter 产生的 `"|..."` leaf，不建立通用 Dynamic tokenizer。

quoted leaf 只接受 Cirru writer 实际产生的四种 escape：`\n`、`\t`、`\"`、`\\`。parser 先验证首尾 quote 与 `|` marker，再在输入上限内分配最多 `input_len - 3` 字节的输出；未知或截断 escape 保持 `ok = false`，最终返回稳定 `Result :err`。

## 验证

- Calcit `:tests` 覆盖空格、换行、tab、quote、backslash 与未知 escape。
- 真实 Wasmtime fixture 将 formatter 的运行时输出重新交给 typed parser 并检查 payload。
- 保持 64 KiB 输入上限与无 trap 错误路径。

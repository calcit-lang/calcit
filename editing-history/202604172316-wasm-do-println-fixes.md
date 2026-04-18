# 202604172316 — WASM `do` body 与 println host import 修复

## 修复内容

### 1. `do` 在 defn body 中的 WASM 编译问题

**现象**：`defn test-println () do (println 42) 1` 编译后生成 `f64.const 0`（函数体为空）。

**根因**：Cirru 语法中 `defn f () do expr1 expr2` 解析为 `(defn f () do expr1 expr2)`，body 数组为 `[do, expr1, expr2]` 三个独立元素（不是 `[(do expr1 expr2)]`）。`do` 作为裸 Import 节点出现在 `emit_expr` 时，触发 "unsupported WASM expression" 错误，导致函数被跳过并 fallback 为 `f64.const 0`。

**修复**：在 `emit_expr` 中为 `Calcit::Import { def: "do" }` 添加特例，将其视为 no-op（emit 0.0 后由 `emit_body` 的 Drop 消耗掉）。

```rust
// `do` as bare body expression is a no-op sequencer
Calcit::Import(import) if import.def.as_ref() == "do" => {
  ctx.emit(f64_const(0.0));
}
```

### 2. io.log_value host import（WASM println）

- WASM codegen 对 `println`/`eprintln`/`echo` 调用 host import `io.log_value`。
- `test-wasm.mjs` 提供 `io.log_value` 实现：读取堆内存判断类型，字符串解码 UTF-8，数字直接输出。
- 添加 `test-println` 测试用例验证功能。

### 3. TypeScript 运行时 BufList 修复

`_$n_buf_list_$o_concat` 中 `CalcitSliceList` 的 `items` 是方法而非属性，需调用 `xs.items()` 并手动迭代 Generator（TS 配置不支持 `for...of` Generator）。

## 受影响文件

- `src/codegen/emit_wasm.rs`：`emit_expr` 新增 `do` no-op 特例；`emit_call_expr` Import 分支也保留 `do` 处理（用于 `(do ...)` 调用形式）
- `ts-src/calcit.procs.mts`：修正 `_$n_buf_list_$o_concat` 迭代逻辑
- `calcit/test-wasm.cirru`：新增 `test-println`
- `scripts/test-wasm.mjs`：新增 `io.log_value` 实现与 `test-println` 断言

## 关键经验

- `do` 在 Calcit defn body 中是语法糖/占位符，预处理后展开为多个独立 body 表达式，不是 `(do ...)` 调用形式。WASM codegen 需将其视为 no-op。
- WASM 编译错误时 fallback 为 `f64.const 0`（静默），调试时需检查 stderr 中的 `[wasm] skipping` 日志。
- TS 的 Generator 不能直接 `for...of`，需手动调用 `.next()`。

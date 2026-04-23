# v0.12.26 WASM 改进总结

## 修改概要

### 1. 修复 `&>=` / `&<=` / `&>` / `&<` 在 WASM 中的 nil=0 碰撞问题

**问题**：Calcit WASM 运行时用 `0.0` 表示 nil，而 `calcit.core/&>=` 等内部函数在比较前会调用 `number? x`，但 `number? 0` 返回 false（因为 nil=0），导致 `assert` trap。

**修复**（`src/codegen/emit_wasm.rs`）：在 `calcit.core` 导入分支拦截 `&>=`、`&<=`、`&>`、`&<`，直接 emit `F64Ge`/`F64Le`/`F64Gt`/`F64Lt` 指令，绕过 nil 检查。

### 2. 修复字符串相等性（`NativeEquals` / `Identical`）

**问题**：`NativeEquals` 使用 `F64Eq` 比较指针，两个内容相同的字符串因指针不同而判断不相等。

**修复**（`src/codegen/emit_wasm.rs`）：改用 `__rt_generic_eq` 运行时函数，进行字节内容比较。

### 3. 修复 `&map:diff-new` 参数顺序

**问题**：实现代码注释写的 "b = args[0], a = args[1]"，但实际语义应为 "a = args[0], b = args[1]"，返回 b 中不在 a 里的条目。测试用例 `(&map:diff-new {:a 1 :b 2} {:b 3 :c 4 :d 5})` 期望 2，但因参数顺序错误返回 1。

**修复**（`src/codegen/emit_wasm/maps.rs`）：调换 `a` 和 `b` 的赋值顺序，使 `a=args[0]`，`b=args[1]`（迭代 b，检查 a）。

### 4. 修复 test-wasm.mjs 字符串测试期望值

**问题**：`test-str-nth` 和 `test-str-first` 的测试期望 byte 数值（101, 104），但 `&str:nth`/`&str:first` 正确返回 1-char 字符串堆指针。

**修复**（`scripts/test-wasm.mjs`）：添加 `readStr` 和 `checkStr` 辅助函数，改用字符串内容比较替代数值比较。

### 5. Clippy 告警修复

**修复**（`src/codegen/emit_wasm/runtime.rs`）：为 `build_rt_hash_f64` 添加 `#[allow(dead_code)]`；为 `build_rt_generic_compare`、`build_rt_generic_eq`、`build_rt_hash_f64_semantic` 添加 `#[allow(clippy::vec_init_then_push)]`。

## 知识点

- WASM nil=0.0 的约束：所有核心函数如果对入参用 `number?` 进行断言，当传入 0 时会 trap。应直接 emit 低级指令绕过。
- 字符串比较应使用 `__rt_generic_eq` 而非 `F64Eq`（指针比较）。
- `&str:nth`/`&str:first` 返回 1-char 字符串（堆指针），不是 char code；test 脚本需用 `readStr` 读取内容比较。
- `&map:diff-new a b` 语义：返回 b 中不在 a 里的条目；参数顺序为 (a, b)。

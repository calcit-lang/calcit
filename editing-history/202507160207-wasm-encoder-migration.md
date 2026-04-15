# wasm-encoder 迁移：WAT 文本 → 二进制 .wasm

## 概要

将 WASM codegen 从手工拼接 WAT 文本字符串改为使用 `wasm-encoder` crate 直接生成二进制 `.wasm`，
同时将测试运行时从 wasmtime CLI 替换为 Node.js 内置的 `WebAssembly` API。

## 动机

- wasmtime 是重量级依赖，CI 需要额外安装步骤，且跨版本 CLI 接口不稳定（v14/v25 breaking changes）
- WAT 字符串拼接容易出错且难以调试
- Node.js 自带 WebAssembly 运行时，零额外依赖

## 关键知识点

### wasm-encoder 用法

- `wasm-encoder = "0.246.2"`，仅 1 个传递依赖 `leb128fmt`
- TypeSection → FunctionSection → ExportSection → CodeSection → module.finish()
- `F64Const` 接受 `Ieee64` 而非 `f64`，需 `Ieee64::from(val)` 包装
- `Function::new(locals_vec)` + `instruction(&Instruction::...)` + `instruction(&Instruction::End)`

### WASM block depth 追踪

- `br N` 中 N 是相对深度（0 = 当前块）
- `if` 块会增加一层嵌套，所以在 `if` 内部 `br` 到外层 `loop` 需要 depth=1
- 解决方案：`WasmGenCtx` 添加 `block_depth` 字段，`emit_if` 时 +1，`recur` 使用 `Br(ctx.block_depth)`

### 两阶段编译

- 函数互调需要预知 fn_index，所以：
  1. 第一遍：收集函数签名和索引映射 `HashMap<String, u32>`
  2. 第二遍：编译函数体，失败的函数用 stub `[f64_const(0.0)]` 保持索引稳定

### 比较运算的 i32→f64 转换

- WASM 比较指令（f64.lt/gt/eq）返回 i32
- 用 `select(1.0, 0.0, cmp_result)` 转为 f64
- `if` 条件用 `f64.ne(val, 0.0)` 转回 i32

## 修改文件

- `src/codegen/emit_wasm.rs` — 完全重写
- `scripts/test-wasm.mjs` — 新增 Node.js 测试运行器
- `scripts/test-wasm.sh` — 简化为生成 + node 调用
- `.github/workflows/test.yaml` — 移除 wasmtime 安装步骤
- `Cargo.toml` / `Cargo.lock` — 添加 wasm-encoder
- `docs/wasm-codegen.md` — 更新文档

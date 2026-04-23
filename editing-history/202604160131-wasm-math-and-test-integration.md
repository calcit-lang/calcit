# 202604160131 — WASM codegen 扩展: 数学函数 + 测试集成

## 新增 WASM 操作

- `floor` → `f64.floor`
- `ceil` → `f64.ceil`
- `round` → `f64.nearest`
- `sqrt` → `f64.sqrt`
- `identical?` → `f64.eq`
- `sin`/`cos`/`pow` 明确报错(WASM 无对应指令)

## 新增文件

- `calcit/test-wasm.cirru` — 17 个纯数值函数测试(fibo, factorial, add-two, sum-range, floor, ceil, round, sqrt, rem, compare, not, let-chain, collatz-steps, gcd 等)
- `scripts/test-wasm.sh` — WASM 验证脚本(生成 WAT → wasmtime compile → 逐函数验证返回值)

## 修改文件

- `src/codegen/emit_wasm.rs` — 新增 `unary_op` 辅助函数, 添加 Floor/Ceil/Round/Sqrt/Identical 分支
- `package.json` — `check-all` 加入 `try-wasm` 步骤; 新增 `try-wasm` 脚本
- `docs/wasm-codegen.md` — 更新支持列表、测试说明、路线图

## 验证

- `yarn check-all` 通过(含 compile, try-rs, try-js, try-ir, try-wasm)
- `cargo test` 246/246 pass
- `cargo clippy -- -D warnings` clean
- wasmtime 逐函数验证: 17/17 assertions pass

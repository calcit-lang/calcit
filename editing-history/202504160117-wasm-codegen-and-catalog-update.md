# 202504160117 — 优化目录更新 + WASM codegen demo

## 优化目录更新

更新 `drafts/04-15-type-directed-optimization-catalog.md`：
- 标记 P0、P1、P2、P5、P7 为已完成，附 commit hash
- 标记 P3、P4 为推迟，附推迟原因
- 更新 Scope 行：从 `TernaryTreeList<ScopePair>` 改为 `Vec<ScopePair>`
- P5 描述更新为实际采用的 Vec 方案（而非原规划的 O(1) 方案）
- 新增「执行记录」节替代原「推荐执行顺序」，含基准数据（836ms→718ms，~14%）

## WASM codegen 实验性功能

### 新增文件
- `src/codegen/emit_wasm.rs` — ~440 行 WAT 代码生成器
- `demos/wasm-demo.cirru` — 示例程序（fibo、factorial、add-two）
- `docs/wasm-codegen.md` — 使用文档 + 未来改进路线

### 修改文件
- `src/codegen.rs` — 注册 `emit_wasm` 模块
- `src/cli_args.rs` — 新增 `EmitWasmCommand`（`cr <entry> wasm`）
- `src/bin/cr.rs` — 新增 `run_wasm_codegen` 入口

### 设计要点
- **纯 WAT 文本输出** — 零额外依赖，不引入 wasm-encoder 等 crate
- **All-f64 类型策略** — 所有值统一为 f64，Bool 用 1.0/0.0
- **支持子集** — defn、if、let、算术(&+/&-/&*/&/)、比较(&</&>/&=)、recur、函数调用
- **不支持** — 字符串、Record/Map/Set、method dispatch、IO、可变参数

### 验证
- wasmtime compile 验证 WAT 合法
- wasmtime run 验证 fibo(10)=89、factorial(10)=3628800、add-two(3.5,2.5)=6
- cargo test 246/246 pass
- cargo clippy -- -D warnings clean
- yarn check-all pass

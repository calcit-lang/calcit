# 整数谓词跨目标一致性（#1454 第一批）

## 范围与依据

基于 #1460 已合并的命名契约，先修行为，再将 `round?/.round?` 迁移到后续目标名 `integer?/.integer?`。这批不实现新名称或 fix 规则，不关闭整个 #1454，也不改变 Number schema。

旧 native 的 `is_integer` 用 EPSILON 容差，误把 ±1e-16 等非零小数当作整数；JS 的 Math.round 比较和 WASM 的 floor 比较把 ±Infinity 当作整数。本次新增的两个 Calcit definition 测试在安装的旧 0.25.1 上均失败，定位到自由函数和方法的近零反例；修复后通过。

## 实现决策

- native 仅调整公开谓词为 `is_finite && fract == 0`，不修改 `src/util/number.rs` 的共享容差工具，避免影响 JSON 表示和索引转换。
- JS 使用 Number.isInteger，区别于 Number.isSafeInteger；超出安全整数范围但仍有限、无小数部分的 Number 保持 true。
- WASM 先求值一次，再判断 `x - trunc(x) == 0`。±Infinity 产生 NaN，比较自然为 false；NaN 与非零小数 false，±0 true。WASI 共用此 lowering。
- 不更改 `round` 的舍入行为、错误参数处理、公开 API 名称或类型。旧应用若依赖近似容差，须显式选择业务精度，不能自动 fix 为舍入或猜测误差阈值。
- CLI `query def calcit.core/round?` 能直接读到新的中文定义契约；升级手册和 API 决策表明确“语义已修复，名称未迁移”。

## 可执行验证

语义预期全部放在 `calcit.core/round?` 的 `:tests`：保留旧 2 条，新增 34 条，覆盖自由函数、Number 方法、±0、正负整数/小数、极小非零值、超出安全整数范围的整数、NaN/±Infinity，以及参数恰好求值一次。Snapshot 仅由 Calcit CLI 编辑，没有文本修改。

`scripts/check-numeric-predicate.mjs` 读取这些测试 AST，复制临时 Snapshot 并通过 CLI 构造执行入口，在 native、生成 JS、core WASM 上重放同一表达式。指定 `WASMTIME_CLI` 时再将入口从 generic export 改为普通 command function（测试体不变），执行真实 WASI 0.3 Component。CI 的现有 core/Component jobs 与 `yarn check-all` 复用该脚本，不新增公共命令或分析器。

- 原生 definition 测试：3/3；共享断言 36 条，native / 生成 JS / core WASM / WASI 0.3 Component 均通过。
- 本机 Wasmtime 49.0.1；参数求值标记在每个目标恰好出现一次且顺序一致。
- `cargo fmt --all --check`、`cargo clippy -- -D warnings`、完整 `cargo test` 通过。
- `yarn check-all` 全量通过，包括 325 个 core definition tests、Agent interface 36/36、既有 native/JS/WASM 与新增共享整数谓词回归。
- `docs check-md`：升级手册 10/10，API 角色文档 7/7；新增代码块已按 CLI 格式化。升级手册原有 9 个非 canonical 代码块未在本次顺带格式化，其示例求值均通过。
- `node --check scripts/check-numeric-predicate.mjs` 与 `git diff --check` 通过。
- 基线 main `b97db04ab7ff8bfa96f73d1beb6815ee8c974c2c` 的 Test / Push on main 已成功。

## 后续边界

WASM `some? false/0` 的 nil 表示问题仍在 #1454：泛型 wrapper 编译后不能仅凭 f64 零值区分 nil、Bool 与 Number。只对两个字面量加例外会漏掉参数、容器和泛型调用；应沿类型证据/单态化处理，未证明的开放边界明确 unsupported。本批没有声称修复它，也没有隐藏对应预期。

然后推进 `non-nil?`、具体成员命题、`integer?` 等命名及 guarded fix；真实消费者的整体迁移仍属于后续 #1454 验收。本次未修改 Respo/js-ffi 项目。

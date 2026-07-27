# 202604251940 — wasm: 新增 test-wasm-suite 多模块入口

## 背景

`test-wasm.cirru` 是手写的小型 WASM 测试用例集，已经通过；但
`yarn check-all` 还会运行 `calcit/test-X.cirru` 一系列测试模块
（test-cond / test-math / test-set / ...），目前 WASM target
还没有专门的入口把这些测试统一跑起来。

## 改动

1. 新增 `calcit/test-wasm-suite.cirru`：手写的 snapshot 文件，
   通过 `:modules` 引入 `util.cirru` 与若干测试模块，自身定义
   `test-wasm-suite.main/main!` 依次调用各测试模块的 `main!`。
   - 当前包含：`util` + `test-cond` + `test-math` + `test-set` + `test-tuple`
   - `test-tuple/main!` 因 `&tuple:enum` 未实现，被 codegen 标为
     skip，main! 会以 `f64 0.0` 占位返回，对整体不产生副作用。
2. 新增 `scripts/test-wasm-suite-extended.sh`：编译并运行新入口的
   一键脚本，输出 `[wasm] skipping ...` 的列表，便于后续逐步补齐
   proc 实现。

## 已知阻塞（短期不放进新入口）

- `test-recursion`：`emit_bump_alloc` 与 closure-as-value 的交互产生
  无效 WAT（`global.set __heap_ptr` 收到 f64），独立编译也会触发；
  与目前 in-progress refactor（emit_set_find_structural / hash_list_or_set）
  联动后表现尤其明显。
- `test-list`：`local.set f64` 收到 i32（function 35 体内），bytes 模式
  形如 `local.get a (i32) ; local.get b (i32) ; local.set c (f64)`。
- `test-string`：`includes?` / `.find-index` / `starts-with?` /
  `strip-prefix` / `strip-suffix` 等字符串过程 WASM 未实现；
  `trim` / `blank?` / `parse-float` / `&str:replace` / `format-to-cirru` /
  `&cirru-quote:to-list` / `get-char-code` 同上。
- `test-fn`：`(let f2 &+)` 把过程作为值放进局部变量，
  当前 WASM 不支持 first-class proc，整个 `main!` 会被 skip。
- `test-algebra`、`test-map`：旧的 `unreachable` 失败，与本次改动无关。

## 用法

```bash
bash scripts/test-wasm-suite-extended.sh
# 期望最后一行: [test-wasm-suite] PASS
```

## 后续步骤

1. 修 test-recursion 的 closure-as-value / bump_alloc 类型不一致
   （定位 `f64.const 0; local.tee Xf64; global.set __heap_ptr` 来源）。
2. 修 test-list 的 i32 ↔ f64 binding bug。
3. 实现 `includes?` / `starts-with?` / `ends-with?` / `strip-prefix` /
   `strip-suffix` / `.find-index` 等 string proc，把 test-string 的
   `test-includes` 子集放进新入口。
4. 实现 `&tuple:enum` 等 tuple 操作，让 test-tuple/main! 不再被 skip。

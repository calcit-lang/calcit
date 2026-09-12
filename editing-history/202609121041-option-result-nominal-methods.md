# 让 core Option/Result 接收者方法走 nominal trait

## 背景

0.14 strict 模式下，core `Option`/`Result` 的接收者方法只能经 `OptionMethods`/`ResultMethods` 这两个 `&impl::new` 生成的 originless inherent bag 派发，被 `E_ORIGINLESS_METHOD_DISPATCH` 拒绝。typed 项目因此必须把 `.unwrap-or`、`.unwrap`、`.some?`、`.and-then`、`.or-else`、`.fold` 等全部改写成 `option:*`/`result:*` 命名函数，与升级手册“接收者已静态推断为 Option/Result 时优先使用接收者 method”的指导冲突（calcit#985）。

## 修改

- 新增内部 nominal trait `OptionOps`/`ResultOps` 及对应 `OptionOpsImpl`/`ResultOpsImpl`，方法签名对齐既有 `option:*`/`result:*` 命名函数的 `:schema`。
- `Option`/`Result` 的 `impl-traits` 改为挂载新的 nominal impl，删除 originless 的 `OptionMethods`/`ResultMethods`。
- `W_NOMINAL_ENUM_LEGACY_USE` 只对 legacy nullable 谓词函数 `some?` 告警；`.some?` 是 nominal Option 方法，不再复用该告警。
- 更新 `type_annotation.rs` 测试里的 impl 占位名，重新生成 `docs/core-dynamic-classification.md` 与 `config/calcit-core-quality.cirru` baseline，移除已删除定义并更新汇总计数。
- `scripts/check-strict-default.mjs` 增加 core Option/Result 接收者方法的 strict smoke 覆盖。

## 验证

- `cargo fmt`、`cargo clippy -- -D warnings`、`cargo test`
- `cargo run --bin calcit -- src/cirru/calcit-core.cirru --compat-types test --tag unit`（238 通过）
- `cargo run --bin calcit -- src/cirru/calcit-core.cirru analyze quality --baseline config/calcit-core-quality.cirru --format json`
- `node scripts/core-dynamic-classification.mjs --check`
- `node scripts/check-strict-default.mjs`
- `cargo run --bin calcit -- calcit/test.cirru --compat-types`
- `yarn compile`、`yarn try-js`、`yarn try-wasm`、`yarn check-static-method-lowering`、`yarn check-js-runtime`
- `bash scripts/check-docs-md.sh`（69 文件 / 331 代码块全部通过）
- 复现命令 `printf 'println $ .unwrap-or (%some 1) 2\n' | calcit exec` 由报错改为输出 `1`

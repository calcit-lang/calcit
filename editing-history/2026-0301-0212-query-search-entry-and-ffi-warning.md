# 2026-0301-0212 query search entry + ffi warning 简化

## 修改概要

本次包含两类改动：

1. `cr query search` / `cr query search-expr` 支持 `--entry <name>`
2. 简化 FFI 回调接口实验性告警（去除低价值 warn）

## 知识点

### 1) search 默认依赖加载来源

`query` 之前只从 `configs.modules` 加载模块。
对于把依赖配置在 `entries.<name>.modules` 的项目，`search`/`search-expr` 可能漏搜。

### 2) entry 级依赖叠加策略

新增 `load_snapshot_with_entry(input_path, entry)`：

- 默认加载 `configs.modules`
- 指定 `--entry` 时，再叠加 `entries.<entry>.modules`
- 对模块路径去重（保持顺序）
- entry 不存在时给出可用 entries 列表

### 3) FFI 回调接口稳定性

`&call-dylib-edn-fn` 与 `&blocking-dylib-edn-fn` 原先标记为 `Experimental`，触发
`registered proc ... is marked as experimental` 告警。

当前 FFI 功能已稳定、该警告噪声较大，因此将这两个接口标记为 `Public`，保留参数校验与平台校验逻辑不变。

## 变更文件

- `src/cli_args.rs`
  - 为 `QuerySearchCommand`、`QuerySearchExprCommand` 增加 `--entry` 选项

- `src/bin/cli_handlers/query.rs`
  - search / search-expr 参数透传 `entry`
  - 新增 `load_snapshot_with_entry`
  - search 输出中显示 `Entry: <name>`

- `src/bin/injection/mod.rs`
  - `&call-dylib-edn-fn` descriptor: `Experimental -> Public`
  - `&blocking-dylib-edn-fn` descriptor: `Experimental -> Public`
  - 注释中移除 experimental 字样

- `docs/CalcitAgent.md`
  - 为 `query search` / `search-expr` 增加 `--entry` 说明

## 验证

- `cargo build --release --bin cr` 通过

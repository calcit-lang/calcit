# 2026-03-05 19:39 query schema 适配

## 改动概要

- 针对 `cr query` 工具链补充 schema 感知与展示，修改文件：
  - `src/bin/cli_handlers/query.rs`

## 具体更新

- `query defs <ns>`
  - 对带 schema 的定义添加 `[schema]` 提示，便于快速识别迁移覆盖率。

- `query def <ns/def>`
  - 新增 `Schema` 区块输出（无 schema 时显示 `(none)`）。
  - `--json` 输出从仅 code 扩展为完整 `CodeEntry` 结构：
    - `doc`
    - `examples`
    - `code`
    - `schema`

- `query peek <ns/def>`
  - 新增 schema 预览（one-liner，超长截断）。

- `query find <symbol>`
  - 引用搜索范围从 `code` 扩展到 `schema`。
  - 输出中标注命中来源 `[code]` / `[schema]`。

- `query usages <ns/def>`
  - 用法搜索范围从 `code` 扩展到 `schema`。
  - 输出中标注命中来源 `[code]` / `[schema]`。

## 兼容性说明

- 未新增 CLI 参数，保持原命令兼容；schema 能力默认生效。
- 仅增强查询展示与搜索语义，不影响运行时求值逻辑。

## 验证

- 已执行并通过：
  - `cargo fmt`
  - `cargo test`
  - `yarn check-all`

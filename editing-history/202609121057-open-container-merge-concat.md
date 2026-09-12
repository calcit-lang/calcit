# 提供开放容器 merge/concat 公开入口

## 背景

0.14 strict 下，`merge` / `concat` 这类要求同质 `Map<K,V>` / `List<T>` 泛型关系的公开 API 无法作用于开放容器：`Map<Tag,Dynamic>` / `List<Dynamic>` 会因 `Dynamic` 占据被重复引用的泛型位置而报 `E_ERASED_GENERIC_RELATION`。此前只能退回低层 raw primitive `&merge` / `&list:concat`，但迁移手册要求 typed code 不把 raw primitive 当作逃生口（calcit#986）。

## 修改

- 新增公开 `calcit.core/merge-dynamic`：契约 `Fn<Map<K,Dynamic>, ...Map<K,Dynamic>> -> Map<K,Dynamic>`，实现复用 `reduce` + `&merge`，不声明同质 value 关系。
- 新增公开 `calcit.core/concat-dynamic`：契约 `Fn<...List<Dynamic>> -> List<Dynamic>`，实现等价于 `concat`，显式保留开放元素边界。
- 为两个定义补充 `:doc`、`:examples` 与 `:tag #{:core :unit}` 单元测试；测试通过 `assert-type` 构造真实 `Map<Tag,Dynamic>` / `List<Dynamic>` 入口，避开 `assert=` 对 Dynamic 容器的泛型关系。
- `scripts/core-dynamic-classification.mjs` 将两个新契约的 5 个 schema-Dynamic 位置登记为 `public-open-boundaries`/`retain-reviewed`，并重新生成 `docs/core-dynamic-classification.md` 与 `config/calcit-core-quality.cirru` baseline。
- `scripts/check-strict-default.mjs` 增加开放容器 `merge-dynamic`/`concat-dynamic` 的 strict smoke 覆盖。
- `docs/run/upgrade.md` 在 `E_ERASED_GENERIC_RELATION` 段落说明开放容器改用这两个公开入口。

## 验证

- `cargo fmt`、`cargo clippy -- -D warnings`、`cargo test`
- `cargo run --bin calcit -- src/cirru/calcit-core.cirru --compat-types test --tag unit`（240 通过）
- `target/debug/calcit src/cirru/calcit-core.cirru analyze quality --baseline config/calcit-core-quality.cirru --format json`
- `node scripts/core-dynamic-classification.mjs --check`、`node scripts/check-strict-default.mjs`
- `cargo run --bin calcit -- calcit/test.cirru --compat-types`
- `yarn compile`、`yarn try-js`、`yarn try-wasm`
- `bash scripts/check-docs-md.sh`（69 文件 / 331 代码块通过）
- issue 复现命令改用 `merge-dynamic` / `concat-dynamic` 后分别返回 `2` / `3`

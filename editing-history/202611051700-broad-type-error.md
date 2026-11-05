# 裸 `Fn` / `Struct` / `Enum` 升级为 `E_BROAD_TYPE`（#2504）

0.35.x 的 `W_BROAD_TYPE` Notes（见 [202611011130-broad-type-advisory.md](202611011130-broad-type-advisory.md)）在 0.36.0 改为错误。上游库（respo、reel、respo-ui、recollect、memof、alerts、ws-edn、feather、markdown、router、message、value）已经用 `AnyFn` / `AnyStruct` / `AnyEnum` 或具体类型清零。

做法：扫描与调用点证据（#2566）不变，仍在加载时入队、检查后合并证据；`snapshot::BroadTypeHit::code` 决定代码。`--check-only` 的 `check_entries` 与逐定义报告 `strict_check::run` 用 `preprocess::split_blocking_advisories` 把 `E_BROAD_TYPE` 从 Notes 中分出：前者整体失败，后者把写它的定义标为 failed（不在可达集合里的定义补一行），依赖它的定义按依赖顺序标为 blocked。运行和 JS 编译的默认证明预检经过 `check_entries`，因此同样拒绝；入口只带 warning 编译通过时，预检也先报告 `E_BROAD_TYPE` 再交给后续流程。没有在加载时直接拒绝，是为了保留调用点证据给出的具体类型 hint。

边界：
- 只报项目自身 namespace，依赖库里的裸写法不报，下游不必等上游发布。内置的 `calcit.core`、`calcit.internal`（含 `calcit.internal.*`）、`calcit.test` 也排除在外（`calcit::is_bundled_core_ns`）：这些底层 proc 本来就对任意 enum / struct 工作。
- 裸 `Trait` 没有 `Any*` 拼写，改成 `Dynamic` 会丢掉族检查，因此保持非阻塞的 `W_BROAD_TYPE`。
- 定义种类误写（`defstruct` 的 `:schema Struct`）同样是错误，改为 `StructDef` / `EnumDef`。
- 诊断码改名为 `E_BROAD_TYPE`，与其他阻塞诊断一致；按代码过滤 Notes 的脚本需要更新。

calcit 自身的测试 Snapshot（`calcit/test-traits.cirru`、`calcit/test.cirru`、`calcit/type-fail/*-strict.cirru` 等 21 个）以及 `tests/fix_cli.rs`、`scripts/check-js-ffi-source.mjs` 里内嵌的代码里的裸写法改为 `Any*`，它们原本测的是开放族语义，改写后语义不变。`preserves-open-callback-spellings` 去掉了 `(:: 'Fn)` 这一旧拼写的比较：它是裸 `Fn` 的另一种写法，现在本身就是错误。

验证：改动的 Snapshot 的 `--check-only --all-defs` 计数与改动前一致且没有 `E_BROAD_TYPE`；`calcit/test-traits.cirru test` 142/142；`cargo test --bin calcit broad`；`cargo test --test strict_check_cli --test human_markdown_cli --test cli_stdout`。；`tests/strict_check_cli.rs` 新增 `bare_broad_types_fail_the_check_and_block_dependents`（fail-fast、运行预检、blocked 传播、`AnyFn` 通过）。

---
title: "Definition-Attached Testing"
scope: "core"
kind: "guide"
category: "features"
aliases:
  - "calcit.test"
  - "calcit test"
  - "affected tests"
  - "test metadata"
---

# Definition-Attached Testing

Calcit stores strict tests beside a definition's code, documentation, examples, schema, and tags. Tests are named metadata entries rather than ordinary definitions, so tools can discover, select, and run them without relying on naming conventions.

Examples remain executable documentation. Tests are CI guardrails: an assertion or preprocessing failure makes `calcit test` exit unsuccessfully, while other selected tests continue unless `--fail-fast` is used.

## Add a Test

Import the built-in assertions where the test expression will run:

```bash
calcit edit add-import app.main --code 'quote $ calcit.test :refer $ is is= is-not= is-throws throws? fail'
```

Attach a stable, named test to a definition:

```bash
calcit edit add-test app.main/add adds-two-numbers \
  --tags unit,fast \
  --code 'quote $ is= 3 (add 1 2)'
```

The expression is compiled in the owning definition's namespace. It can use that namespace's imports and local definitions.

Use a file or stdin for a multiline expression. As with other AST edit commands, the input must contain exactly one quoted node.

## Inspect and Maintain Tests

```bash
calcit query tests app.main/add
calcit query context app.main/add --format json
calcit edit add-test app.main/add adds-two-numbers --overwrite --code 'quote $ is= 5 (add 2 3)'
calcit edit rm-test app.main/add adds-two-numbers
```

Test names must be non-empty, have no surrounding whitespace, and be unique within one definition. Replacing one requires explicit `--overwrite`; removal uses the stable name rather than an array index.

The persisted shape is equivalent to:

```cirru.no-check
:tests $ []
  %{} 'TestEntry
    :name |adds-two-numbers
    :code $ quote $ is= 3 (add 1 2)
    :tags $ #{} :unit :fast
```

Snapshots written before this field existed load with an empty test list.

## Run Tests

```bash
# All project-owned tests
calcit test

# One namespace or definition
calcit test app.main
calcit test app.main/add

# Exact name and tags
calcit test app.main --name adds-two-numbers
calcit test --tag unit --tag fast

# Exclude slow or integration guards, and reject an empty selection
calcit test --tag unit --exclude-tag slow --exclude-tag integration --require-match

# Inspect selection without executing
calcit test app.main --list

# Emit a compact machine-readable report for large CI or agent runs
calcit test --tag unit --summary-only --format json
```

Each test is compiled as its own synthetic function and executed independently. Reports use the stable identifier `namespace/definition#test-name`.

Without a scope, `calcit test` discovers tests only in namespaces defined by the input snapshot. Tests bundled with `calcit-core.cirru` or loaded modules are excluded, so an external project does not accidentally run Calcit's own test suite. Pass an explicit namespace such as `calcit test calcit.test` when maintaining the core assertions.

In JSON mode, runner output produced by `println`/`echo` is redirected to stderr so stdout remains one parseable report envelope. Full reports contain one row per selected test, including its execution duration. `--summary-only` suppresses per-test program output and emits counts and total duration only, with `detail: "summary"`; failures are still reported by the test runner. This is useful when an agent needs a reliable gate without loading hundreds of passing test rows. The report also distinguishes `selected` tests from `executed` tests when `--fail-fast` stops early.

`--exclude-tag` removes a test carrying any supplied tag, after required `--tag` matching. `--require-match` turns an empty selection into a failure; use it in CI whenever a tag, scope, or affected-test command is expected to protect a non-empty suite.

## Run Affected Tests

```bash
calcit test --affected app.math/add
calcit test --affected app.math/add --affected app.math/subtract
calcit test --affected app.math/add --list
```

For affected selection, Calcit preprocesses every candidate test and follows the compiled `DefId` dependency graph transitively. It always includes tests attached directly to a changed definition. If dependency analysis for a test fails, that test is conservatively selected and reported as failed, so a static-analysis problem cannot silently hide a failing guardrail.

Normal `calcit` execution does not run tests implicitly. CI and coding agents should invoke `calcit test` explicitly.

## Built-in Assertions

The `calcit.test` namespace is embedded in `calcit-core.cirru` and requires no external module:

- `is` asserts a truthy expression.
- `is=` and `is-not=` compare values.
- `throws?` reports whether an expression raises.
- `is-throws` requires an expression to raise.
- `fail` raises immediately with a message.

All assertion macros evaluate each supplied expression once.

## Execution Cost

Normal `calcit test` runs compile each selected test only when it is about to run.
This keeps `--fail-fast` responsive and avoids preprocessing tests after the
first failure. `--affected` intentionally compiles its candidate tests first:
it needs their static dependency graph to select a safe, transitive subset.

Tests execute the selected test and its runtime dependencies; documentation
and examples are not themselves executed as tests. Prefer a short
definition-local test for one API contract, and reserve integration fixtures
for behavior that actually crosses definitions or backends.

## Target Coverage

### 仓库 CI 与快速本地回归

CI 的 Core/CLI 与文档检查共用一次 `ci` 配置构建的 CLI：开启优化，同时保留
debug assertions 与整数溢出检查。同一 workflow 内按提交 SHA 命名的 artifact
供两个任务下载使用，不跨提交寻找旧二进制。脚本通过 `CALCIT_BIN` 选择 CLI；
未指定时保留各脚本原有的本地构建查找方式。

```bash
cargo build --locked --profile ci --bin calcit
yarn compile
yarn procs-link
export CALCIT_BIN="$PWD/target/ci/calcit"
node scripts/run-core-tests.mjs
node scripts/check-strict-default.mjs
node scripts/check-known-assertion.mjs
bash scripts/test-wasm.sh
bash scripts/check-docs-md.sh
```

这些命令用于对应范围的回归，不代替完整 CI。普通 core 语义由统一运行器覆盖；
`CALCIT_LINT_CORE=1` 的直接执行另行保留，用于检查改写后的树。
`post_lowering_cli` 和 namespace-import 集成测试在 Rust 测试任务中运行，后者
同时实际执行生成的 JS，不再在 Core/CLI 任务中重复编译和执行 Rust 测试。
断言检查脚本保留 fixture 专属回放、非法类型程序的逐后端诊断以及失败时不产出
代码的检查，不再复制统一运行器已覆盖的 core 定义回放。

PR 推送新提交时，CI 取消同一 PR 旧提交上尚未完成的 Test workflow；main
的每次提交仍独立验证。减少执行成本时，应先移除重复构建与重复回放，保留
原始 `:tests`、零匹配失败检查和独立的后端边界验证。

### 统一后端回放

`calcit test` 在 native 上运行 definition `:tests`。仓库用一个统一运行器把同一批 `:tests` 在 native、生成的 JS 与 WASM 上各执行一次：

```bash
# 默认：全部 core :tests，native / JS / WASM
node scripts/run-core-tests.mjs

# 按 tag、名称或定义缩小范围；零个测试被选中时失败
node scripts/run-core-tests.mjs --tag unicode --backend native,js
node scripts/run-core-tests.mjs --target 'calcit.core/round?' --name distinguishes-integers

# 其它 Snapshot 的 :tests 也可重放；WASI 0.3 command 需显式选择并提供 Wasmtime
node scripts/run-core-tests.mjs --snapshot calcit/test-traits.cirru --target test-traits.main/test-qualified-contains-boundary --backend native,js
WASMTIME_CLI=wasmtime node scripts/run-core-tests.mjs --backend wasi --target 'calcit.core/round?'
```

运行器为每个测试生成一个零参数函数，写入临时 Snapshot，再分别执行：native 与 WASI 逐个打印标记后运行，JS 导入生成的模块逐个调用，WASM 在 Node 中实例化后逐个调用导出函数。native 是参照结果，JS 与 WASM 的 `println` 输出必须与 native 一致。失败报告列出测试 id、后端、期望与实际值，以及该测试在其它后端的状态。

新增的 core `:tests` 自动进入三个后端，无需登记。某个后端暂时无法运行的测试写入 `scripts/core-tests-exclusions.cirru`，按后端列出测试 id 或 tag，并写明原因，原因以类别开头：`unsupported`（后端明确拒绝该构造）、`parity`（结果与 native 不同，另开 issue 修复）、`host`（需要回放宿主未提供的能力）、`replay`（因不在所属 namespace 回放而产生）。清单中引用不存在的测试会使运行失败；`--report-unexpected-pass` 会额外运行被排除的测试，列出已经通过、可以移出清单的条目。

新增测试需要核对它实际使用的后端能力。例如 `try`、`atom` 和开放值的 `data-view` 目前在 WASM/WASI 明确报告 unsupported；对应测试仍完整运行于 native/JS，排除项必须指向具体测试并记录实际诊断。一个测试同时覆盖已支持和未支持的构造时，保留原测试，并给已支持的断言单独附加 `:tests`，避免因整项排除而丢失这部分跨后端覆盖。不能用排除项掩盖本应支持的行为回归或改变原断言。

只测试编码、ABI、memory layout、宿主注入或 unsupported 诊断的场景继续使用 `scripts/test-wasm*.mjs`、`scripts/check-*.mjs` 等专用脚本；新的跨后端回放需求给 `:tests` 加 tag 并用统一运行器选择，不再新增 `scripts/check-*.mjs`。

运行器拒绝空的或重复的后端列表。native 与 WASI 的每个测试必须完整输出开始和结束标记；进程提前正常退出不会被计为通过。`--report-unexpected-pass` 自动补上 native 参照，只有测试成功且输出与 native 一致时才建议移除排除项。

### 限制

- WASM 在独立的回放 namespace 中执行 core 测试，依赖 `calcit.core` 内部豁免的测试列为 `replay`。
- WASM 宿主只提供 `io.log_*` 与 `math` 导入，其它宿主调用会使测试失败。
- WASI backend 不在默认集合中，需要 `WASMTIME_CLI`。

## Choose the Test Surface (Calcit-first)

Place user-observable behavior in definition `:tests`; keep Rust tests for the
low-level boundaries a Calcit program cannot reasonably construct.

- Prefer `:tests` for language semantics, type inference, macro expansion, core
  APIs, runtime behavior, and cross-backend contracts. Write the same
  expressions a user would, so reviewers see the input and expected result
  directly.
- Keep Rust tests for parsers/serializers, internal data structures, host/FFI
  boundaries, memory or concurrency invariants, error recovery, and conditions
  without a stable Calcit surface.
- A backend-specific implementation may keep precise Rust tests, but should also
  have a Calcit-side test for the shared semantic. Add target execution or
  codegen evidence when JS, IR, or WASM is involved.
- When behavior changes, migrate a Rust test to `:tests` if it only verifies
  semantics Calcit can express; remove the duplicate once Calcit coverage is
  equivalent or stronger.
- Do not add Rust/Calcit test-count ratios, coverage percentages, or a
  statistics analyzer. Placement follows reviewable semantic boundaries.

例如，严格 Tag 语义把用户可观察的 Tag/String 区分写在 `calcit.core/&compare`
的 `:tests` 中。WASM 若复用这项语义，应执行同一份 Calcit case；只有编码、ABI、
memory layout 或 unsupported lowering 这类 backend 内部边界才保留 Rust/脚本 fixture。

### PR checklist

- Why does this coverage belong in `:tests`, or what concrete low-level reason
  keeps it in Rust?
- Do the Calcit tests assert the relevant success, failure, or boundary branch
  with user-level expressions?
- If a Rust test only restates the same external semantics, was it migrated or
  removed?
- Do Native and the relevant JS/IR/WASM checks still pass?

## Core Test Placement

For a core function, macro, or builtin whose behavior can be expressed in one
namespace, attach the test directly to that definition in `calcit-core.cirru`:

```bash
calcit src/cirru/calcit-core.cirru edit add-test calcit.core/range creates-half-open-range \
  --tags unit,core \
  --code 'quote $ assert= ([] 2 3 4) (range 2 5)'
```

The repository runs this suite with `yarn try-core-tests`, which explicitly
loads `calcit-core.cirru` and runs its `:unit` tests in summary mode. When the
input snapshot itself contains `calcit.core`, `calcit` preserves that source
namespace instead of replacing it with the binary's embedded copy. This makes
the command reliable even when the globally installed `calcit` predates the source
snapshot being tested. Keep `calcit/test-*.cirru`
when a case verifies several definitions together, parser syntax, stateful
behavior, JavaScript/WASM code generation, or a full program flow. Those files
are integration fixtures rather than a substitute for definition-local tests.

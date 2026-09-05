## 开发与验证流程

### 核心步骤

- **代码规范**：执行 `cargo fmt` 保持代码格式一致性。
- **质量检查**：执行 `cargo clippy -- -D warnings` 消除潜在风险与性能问题。
- **构建验证**：执行 `yarn compile` 确保前端 TS 与 Rust 核心构建正常。
- **运行测试**：执行 `cargo test` 验证单元测试，`yarn check-all` 验证全量集成测试。
- **Agent CLI 协议检查**：在本仓库执行 `yarn check-agent-interface`，验证查询命令 stdout 可解析为单个 JSON，并记录语义查询耗时与输出字节数。该命令属于 Calcit 仓库开发流程，不适用于普通业务项目。
- **外部项目回归**：CLI 查询、编辑或类型分析改动完成后，用已全局安装的新 `calcit` 在 Respo 等真实项目验证；有写入风险的编辑命令先作用于 Snapshot 临时副本。大项目只需要统计时使用 `--summary-only`，examples 回归优先使用 `check-examples --ns <ns> --def <definition>`。

### 功能准则

- **明确边界**：功能改动需考虑边界条件、错误行为及兼容性。
- **一致性**：复用现有模式，保持日志和错误信息风格统一。
- **测试覆盖**：新功能必须补齐正常路径与异常分支的测试用例。

### 语言定位与文档一致性

- **以 Calcit 自身为中心**：文档直接介绍 nominal struct/enum、traits 与方法、Option/Result、静态分析、typed FFI、Cirru source model 和跨 backend 语义，不再以 “ClojureScript dialect” 或其他语言类比作为主要解释框架。
- **历史比较只服务迁移**：Clojure/ClojureScript 等历史影响可在迁移提示中保留；只有在避免具体语法、参数顺序或语义误判时才使用比较，不把外部语言当作设计依据。
- **统一实时应用模型**：面向 Web 应用的设计围绕 Calcium Workflow：typed operation/message envelope、串行确定性 updater、Respo/Recollect projection 与 diff/patch、revision/ack/resync、有界异步、可观测收敛。
- **能力优先以方法暴露**：新增公共抽象优先通过 trait 与 method 形成一致 API；先强化 nominal types 和 typed boundaries，再考虑增加语法或借鉴其他语言特性。
- **跨项目验证**：相关语言/生态改动优先在 Calcium Workflow 回归，并逐步用 TopixIM/Timegrass 类真实实时应用验证；Calcit、Respo、Recollect、WebSocket 与 native FFI 模块应保持层次清晰、职责一致。
- **双语协作记录**：关联 Issue 与 PR 的标题、正文和阶段性进度保持中英双语，便于跨项目追踪同一设计方向。

### 仓库职责与拆分模块追踪

- **主仓库边界**：parser/source model、Snapshot、preprocess/type system、runtime、JS/WASM/Calx backend 语义、`@calcit/procs`、`calcit` CLI/Agent interface 以及权威 RFC/离线文档留在本仓库。短期不规划 LSP，不为假设中的 LSP consumer 拆独立 analysis 仓库。
- **拆分总索引**：跨仓库职责和迁移顺序由 [calcit#549](https://github.com/calcit-lang/calcit/issues/549) 追踪；bindgen、caps、Calx benchmark 和 native ABI 的具体任务使用总索引中的 child Issues，避免重复立项。
- **生态 Wiki 外置**：生态目录、边界、依赖层次和批量演进说明只维护在独立的 [`calcit-lang/calcit` GitHub Wiki](https://github.com/calcit-lang/calcit/wiki)，不在主仓库保存正文副本或自动镜像脚本。Wiki 用于发现和导航，各模块仓库 README/AGENTS.md、主仓库版本化文档与测试仍是具体契约的 source of truth。
- **状态必须可发现**：每个拆出或供多个仓库复用的模块，都必须在自己的 README 或 AGENTS.md 说明状态（production / experimental / template / internal）、职责与非职责、上游/下游契约和 source of truth、兼容矩阵、版本与发布策略、迁移/验证命令以及关联 umbrella/child Issues。
- **模板不冒充产品**：workflow/template 仓库必须明确标记用途及“不随业务功能迭代版本”；实验性 benchmark 也必须明确结果可比性和非生产定位。
- **Calx profile 证据**：机器相关的 Calx 采样策略、raw reports 与性能 provenance 由 [`calcit-lang/calcit-calx-bench`](https://github.com/calcit-lang/calcit-calx-bench) 独立维护；core 的 `docs/run/calx-compile-cache.md` 只拥有 cache/runtime 语义与设计约束。报告必须记录干净 commit、环境、命令、迭代数、原始文件哈希与 inclusive-stack 限制，不把原始 profiler 资产提交回 core。
- **迁移完成才删除**：只有目标仓库具备文档、Actions、发布或实验运行入口、兼容验证与跨仓库 smoke 后，才能从主仓库删除原实现；迁移期 README 必须同时说明当前入口与目标状态。
- **Bindgen 契约**：拆分已完成；core 只拥有 `calcit ffi export`、版本化 Interface IR schema、导出语义和最小 conformance tests。确定性 Rust/Calcit/TypeScript/WIT generation、compatibility diff、manifest、stale check、WIT validation 与 capability matrix 以 [`calcit-lang/calcit-bindgen`](https://github.com/calcit-lang/calcit-bindgen) 为准；不得把 generator preview、golden 或 WIT tooling 重新加入 core release。
- **Calx harness 契约**：拆分已完成；core 的 `docs/run/calx-harness-extraction.md` 只保留 ownership/discovery，产品契约、`pins.json`、运行方法与报告 schema 以 standalone 仓库为准。lowering/correctness 留在 core，外部 harness 必须 pin Calcit revision，不能依赖可变全局或把机器阈值写成 correctness gate。
- **Calx session adapter**：外部 harness 只能经 `calcit::codegen::calx::benchmark_session` 使用固定 revision 的内部接口，并记录 `CALX_BENCHMARK_SESSION_EDITION`；不得重新引入 `PROGRAM_CODE_DATA`、`ProgramFileData`、`ensure_def_id`、`run_fn` 或其他 mutable-global 访问。

直接使用命令修改 calcit 程序时不需要调用 cargo, 直接按照文档给出的命令行示例执行即可。

在开始任何 `calcit edit` / `calcit tree` 修改前，先把下面这条命令当作**硬前置步骤**执行一遍，而不是可选建议：

```bash
calcit docs agents --full
```

未先阅读最新 Agent 指南时，不要直接开始改 `calcit.cirru`；旧文件名 `compact.cirru` 已停用，必须先按升级指南迁移。

### 运行模式更新（calcit / js）

- `calcit <entry>`、`calcit <entry> js` 默认都是**单次执行**（once）。
- 需要监听时，显式传 `-w` 或 `--watch`（如 `calcit -w <entry>`、`calcit <entry> js -w`）。
- `calcit <entry> ir` 仅用于编译器与生成结果调试，不作为普通项目的运行或验证方式。
- WASM codegen 是仓库内部验证后端，不向用户提供命令；维护者统一通过 `yarn try-wasm` 验证。

### calcit eval 基础与常见踩坑

- **用途定位**：`calcit eval` 适合快速验证语义/类型提示与宏展开，不等同于完整项目运行。
- **可加载外部模块**：`calcit eval` 支持重复传入 `--dep`，可加载多个模块目录（路径以 `/` 结尾时读取其中的 `calcit.cirru`；仅有 `compact.cirru` 的模块会被拒绝并提示迁移）。
  - ✅ `cargo run --bin calcit -- calcit/test.cirru eval --dep ~/.config/calcit/modules/respo.calcit/ -- 'ns app.demo $ :require respo.util.detect :refer $ element?\n\nelement? nil'`
- **首表达式 `ns` 会注入当前 eval 程序**：当 snippet 第一个表达式是 `ns` 时，会把 `ns <NS> ...` 从第 3 个节点开始（通常是 `:require` 等规则）合并到运行用的 `ns app.main`，用于在 eval 中显式导入命名空间。
- **`docs check-md` 也支持依赖模块**：`calcit docs check-md` 可通过多次 `--dep` 传参，内部会透传给 `eval`/`--check-only`。这样 markdown 代码块可配合首行 `ns ... :require ...` 访问模块函数。
  - ✅ `cargo run --bin calcit -- calcit/test.cirru docs check-md docs/CalcitAgent.md --dep ~/.config/calcit/modules/respo.calcit/`
- **顶层无需额外括号**：Cirru 语法本身就不需要"最外层括号"，顶层可以直接是表达式。可用 `calcit cirru parse -e` 观察解析结果。
  - ✅ `cargo run --bin calcit -- calcit/test.cirru eval 'range 3'`
  - ✅ `cargo run --bin calcit -- calcit/test.cirru eval 'let ((x 1)) (+ x 2)'`
  - ❌ `cargo run --bin calcit -- calcit/test.cirru eval '(range 3)'`（多一层括号会改变调用语义）
- **`let` 绑定语法**：必须用成对列表，形如 `((name value))`。
  - ✅ `let ((x 1)) x`
  - ❌ `let (x 1) x`（会触发"expects pairs in list for let"）
- **`$ (expr)` 双重求值陷阱**：在 `let` 绑定中，`x $ (f a)` 会被解析为"先求 `(f a)` 再以结果为操作符再调用一次"，触发"cannot be used as operator"。原因是 `$` 后接 `(...)` 形成了两层调用。正确写法是省略 `$`，直接写 `x (f a)` 或 `x f a`。独立一行的 `(expr)` 同理（等同于把结果再调用一次），需加前导 `,` 或改写为非括号形式。
- **末尾符号被当作函数调用**：在 `fn` 或 `let` 的最后一行，单个符号（变量名）会被当作调用（如 `acc` → 触发 "cannot be used as operator"）。需用 `, acc` 加逗号前缀传递值，或将其包在 `println` 等函数调用中返回。
  - ❌ `fn (acc item) if flag (acc) acc`（末尾 `acc` 被当作调用）
  - ✅ `fn (acc item) if flag (acc) , acc`（`, acc` 表示"按值传递"）
- **`foldl` 初始空集合语法**：`foldl xs [] $ fn ...` 中，`[]` 会因 `$` 右结合被解析为 `([] (fn ...))` 而非空列表。正确写法是先绑定 `init $ []`，再传 `init`；或对空 map 同理使用 `init $ {}`。
- **告警会使 eval 失败**：有类型告警时，`calcit eval` 会以错误退出（这是预期行为，便于阻断不安全用法）。
  - 例：`cargo run --bin calcit -- calcit/test.cirru eval '&list:nth 1 0'` 会提示 `:list` vs `:number` 的类型告警。
- **JS FFI 动态访问告警（opt-in）**：传 `--warn-dyn-method` 时，裸 `JsObject` 上的 `.-`/`.!`/`aget`/`aset`/`js-get`/`js-set`（静态字面量 key）会额外报告 `W_JS_FFI_UNTYPED_ACCESS`，提示声明 external-object trait 提升类型覆盖度；不传 flag 时静默。
- **assert-type 仅做检查**：`assert-type` 在预处理阶段生效，不会改变运行值。
  - 例：`cargo run --bin calcit -- calcit/test.cirru eval 'let ((x 1)) (assert-type x :list) x'` 依然返回 `1`，并在检查阶段报告类型不匹配。
- **常用排错方式**：遇到报错先看 `.calcit/error.cirru`，它会提供更完整的栈信息。
- **查示例用法**：可用 `calcit query examples <namespace/definition>` 查目标定义的示例。
  - 例：`cargo run --bin calcit -- calcit/test.cirru query examples calcit.core/let`

### CLI 修改指南与约束

- **优先使用 `search-replace`**：在 `calcit tree` 操作中，优先使用 `search-replace` 而非 `replace`。它基于内容定位，且在不唯一时会报错，比手动指定索引更安全。
- **全量取消 `--stdin` 支持**：由于 Shell 重定向和多行输入的复杂性，所有的修改类子命令（`edit` 和 `tree` 系列）已移除该选项。
  - ✅ 使用 `--code 'code'` 进行单行输入（自动检测 JSON vs Cirru）。
  - ✅ 使用 `--file file` 进行多行或复杂结构输入（推荐在 `.calcit/snippets/` 下创建临时文件）。
  - ✅ 省略 `--code` `--file` 时自动从 stdin 读取（推荐用于多行代码和避免转义）。
- **路径索引动态性**：在 `tree` 系列操作中（如 `delete`, `insert`），操作会引起同级后续节点索引变化。建议**从后往前**操作，或每次修改后使用 `query search` 重新定位。
- **结构引用替换 (`tree rewrite`)**：`tree replace` 仅支持简单替换。涉及引用原始节点及其内容的复杂替换（使用 `--with name=path`）已统一移动至 `calcit tree rewrite` 命令。
- **常用的编辑操作**：
  - `edit mv-def <source> <target>`：移动定义到另一命名空间；同命名空间重命名用 `edit rename <source> <new-name>`。
  - `edit cp <target> --from <path> --path <path> [--at <pos>]`：在 AST 节点间复制内容，支持 `before`, `after` (默认), `prepend-child`, `append-child`, `replace`。
- **`calcit edit` 命名空间 import 操作格式**（三个命令的 `--code` 输入格式不同，混淆会导致静默损坏）：
  - `edit add-ns <ns>`：推荐不传 `--code`，创建空 ns 再逐条 `add-import`。若传 `--code`，必须是完整 `ns` 表达式且内部名称与位置参数完全一致。
  - `edit imports <ns> --code 'src-ns :refer $ sym'`：**不含 `:require` 前缀**，直接是规则体；多条规则用 `--file file`（每行一条）或 stdin 传入 JSON 数组 `[["src",":refer",["sym"]],...]`。
  - `edit add-import <ns> --code 'src-ns :refer $ sym'`：格式与 `imports` 单条规则相同；已存在同名来源时加 `--overwrite` 覆盖。
  - 优先用 `add-import`（带校验和覆盖保护），`imports` 只在需要全量重置所有 import 时使用。

## 发布流程规范

### 版本升级

升级版本时需同步更新两处，缺一会导致 crates.io 或 npm 发布不一致：

- 修改 `Cargo.toml` 中的 `version` 字段
- 修改 `package.json` 中的 `version` 字段（保持一致）
- 执行 `cargo update --workspace` 更新 `Cargo.lock`

### PR 与发布流程

1. **先合并功能改动到 main**：功能分支完成后推送并等待 GitHub Actions 全绿；确认稳定后通过 PR 合并到 `main`。
2. **升级版本**：版本号更新允许直接在已验证的 `main` 上进行，不需要为版本号本身创建 PR。同步更新 `Cargo.toml`、`package.json`，执行 `cargo update --workspace`，提交 `chore: release <version>` 并推送 `main`。如果项目维护者希望版本变更单独审核，也可以自愿使用 `codex/release-<version>` 分支和 PR，但不是硬性要求。
3. **准备发布提交**：release PR 合并后拉取最新 `main`，确认版本提交确实位于 `main` HEAD 且目标 tag 尚不存在。可以先检查 Actions 状态，但不要求等待所有 Actions 完成后再继续。
4. **从 main 打 tag 并发布**：直接在已同步的 `main` 上创建并推送不带 `v` 前缀的 tag，然后创建 GitHub release；这一步不需要再创建发布分支。打 tag 本身可以在 Actions 尚未完全验证时进行。这一步触发 `publish.yaml` 自动发布到 crates.io 和 npm，之后继续轮询并确认发布 workflow 成功。
5. **最终确认发布成功**：轮询 GitHub Actions 直到 publish workflow 成功，并在 crates.io / npm 上确认新版本可见（版本号一致）。

```bash
# 版本号更新（允许直接在已验证的 main 操作；不要求创建 release PR）
git switch main
git pull --ff-only origin main
# 修改 Cargo.toml、package.json，随后：
cargo update --workspace
git add Cargo.toml package.json Cargo.lock
git commit -m "chore: release 0.13.8"
git push origin main

# release PR 合并后回到 main，再打 tag 并推送（不带 v 前缀；不再创建 tag 分支）
git switch main
git pull --ff-only origin main
git tag 0.12.28 -m "Release 0.12.28"
git push origin 0.12.28

# 创建 GitHub release（触发 publish.yaml）
gh release create 0.12.28 --title "0.12.28" --notes "..."

# 验证 publish workflow 状态（用轮询，不要用 gh run watch——它是交互式 TUI，会卡住脚本）
gh run list --limit 5

# 发布后抽样确认远端版本（示例）
cargo search calcit --limit 1
npm view @calcit/procs version
```

> ⚠️ **`gh run watch` 是交互式 pager（类似 less），在脚本或 Agent 场景下会卡住**，按 `q` 退出后续命令也不会被执行。验证时统一用 `gh pr checks <pr-number>` 和 `gh run list --limit 5` 轮询。

## 性能与资源验证

### 技术基准

- **启动耗时验证**：使用 `time ./target/release/calcit calcit/add.cirru`，与改动前同平台 release 基线比较。若数值大幅增加，需检查 `build.rs` 的序列化或 `include_bytes!` 是否失效；旧 `-1` once 参数已经移除。
- **构建体积监控**：使用 `stat -c '%s %n' target/release/calcit`（macOS 系统 `stat` 可用等价参数）记录精确字节数，并与改动前同平台 release 基线比较；当前 arm64 产物约 8 MiB，不再使用过时的 5 MiB 绝对阈值。引入新依赖前务必检查其 Transitive Dependencies，优先选择同步轻量库（如 `ureq`）。
- **IO 纯净度检测**：确保 stdout 仅保留程序逻辑输出。版本信息、预热日志、告警均应通过 `eprintln!` 输出至 stderr。验证：`./calcit -v > /dev/null` 不应有任何输出。

### 内存性能检查

- **高频分配规约**：在 Preprocessing 阶段，严禁在循环内调用 `Arc::new(CalcitTypeAnnotation::Dynamic)`。应始终 clone 单例 `crate::calcit::DYNAMIC_TYPE`。
- **验证手段**：对于大规模 Cirru 项目的预处理，可通过 `repeat 10 { time ./target/release/calcit ... }` 观察耗时抖动。若抖动剧烈，通常预示着堆内存申请频率过高或冷热数据加载策略存在问题。

### JS 代码生成可读性

- JS codegen 采用**行级缩进**（复用 `indent_block`，O(n) 单遍处理），不做 AST 级美化：函数体与嵌套语句块缩进一级，顶层函数间插入空行。改动 codegen 时保持这一低成本风格即可，不要引入完整格式化器。

## 项目结构概览

- `src/`：Rust 核心实现（`src/calcit/` 数据结构, `src/runner/` 运行时, `src/builtins/` 语法, `src/codegen/` IR/JS/WASM 输出）。
- `calcit/`：Cirru 源码与测试用例（`*.cirru`）。
- `lib/` & `js-out/`：JS 共享库与编译输出。
- `docs/` & `demos/`：开发文档与实验性示例。

### 编辑历史

每次 commit 之前, 都产生一个时间戳(年月日时分)开头的文件, 记录本次修改的知识点和概要, 方便未来修改类似功能时查阅. 统一存储在 `editing-history/` 目录.

## 多 Agent Issue 协作

`calcit-lang/calcit` 的 GitHub Issues 是 Calcit 主项目及关联仓库工作的唯一协调状态源。即使代码实际修改在 `calcit-bindgen`、`calcit-calx-bench` 等关联仓库，也必须先在本仓库建立或关联一个主 Issue，并在其中写明所有目标仓库和路径。

### Release manager / 协调 Agent

每轮发布指定一个 release manager 负责协调，不直接认领或实现普通 `agent:ready` Issue，不进入实现 Agent 的 worktree，也不修改 Issue `Owned scope` 中的源码。release manager 的职责是维护 GitHub Issues、PR 状态、依赖关系、milestones、优先级和发布门槛，并推动其他 Agent 在各自 worktree 中完成工作。

- 所有开放 Issue 都必须归入一个 milestone；新 Issue 出现后立即判断属于当前版本、后续版本或无截止日期的长期迭代，不能长期处于无 milestone 状态。
- 每个 milestone 必须有双语目标、截止日期（长期迭代可省略）和可验证的退出条件。当前版本只包含确实阻塞该版本的工作；未完成事项在发版前移动到下一 milestone，并在 Issue 留言说明理由，不得为清空 milestone 而虚假关闭。
- release manager 维护状态一致性：只有依赖满足且范围可执行的 Issue 使用 `agent:ready`；持有有效租约时使用 `agent:claimed`；已有覆盖最新提交的 PR 时才使用 `agent:review`；等待明确依赖、权限或人工决策时使用 `agent:blocked`。
- release manager 定期检查当前 milestone 的开放 PR、required checks、review threads、mergeability 和依赖变化。只在状态变化、失败、冲突或需要行动时更新 Issue，状态不变时不重复留言或紧密轮询。
- 当前版本建立独立的 release Issue，并标记 `priority:p0`。依赖未完成时保持 `agent:blocked`；所有版本内实现 Issue 已关闭或明确移出、required PR 已合并且 release 验收条件满足后，release manager 才把它切换为 `agent:ready`，交给另一个 Agent 认领和执行发布。
- release manager 不用实现租约来修改 GitHub milestone、Issue 标签、依赖、状态和协调评论；但任何仓库文件改动（包括已经合入后的 `AGENTS.md`、workflow 或 release script）仍必须建立 Issue，由实现 Agent 认领、在独立 worktree 修改并创建 PR。
- release manager 不把“分支已 push”视为进度完成。发现分支有新提交但没有覆盖该提交的开放 PR 时，将对应 Issue 退回 `agent:ready` 或标记 `agent:blocked`，并要求实现 Agent 创建 PR；release manager 自己不接管该分支补实现。
- tag、GitHub release、crates.io/npm 发布均验证成功后才能关闭 release Issue 和对应 milestone，并立即确认下一 milestone 成为唯一的当前迭代。

仓库维护者首次启用时执行 `scripts/agent-issue-lease.sh init` 创建四个状态标签。执行认领的环境必须具备可 push `agent-lock/*` 分支、编辑 Issue 和标签的 `git`/`gh` 权限。

### 开始写入前

1. 只处理开放且带 `agent:ready` 标签的 Issue；先阅读 Issue、依赖项和关联 Issue。
2. 为本次运行选择全局唯一且不易碰撞的 ID，建议包含客户端、完整任务/会话标识和随机后缀，例如 `codex-01a06d08-654-a91f`。不得只截取很短的公共前缀（如 `codex-01a06d`），也不要复用其他正在运行的 ID。
3. 在本仓库执行：

   ```bash
   scripts/agent-issue-lease.sh claim <issue-number> <agent-id> '<repo:path, repo:path>'
   ```

4. 只有命令显示 `CLAIMED` 或 `RENEWED` 后才能修改文件。认领失败时立即停止，不得绕过锁。
5. 每个 Agent 同时只持有一个写入租约，但可以跟踪多个已进入 `agent:review` 的 Issue/PR。不同 Issue 的修改范围如果重叠，也不得并行写入；在两个 Issue 留言说明冲突后等待拆分或释放。

远端 `agent-lock/issue-<number>` 分支是认领权的权威来源；Issue 标签与租约评论用于人员和 Agent 查看。不要手工创建、覆盖或删除该分支。脚本使用原子 Git ref 更新防止同时认领，并能在租约过期后安全接管。

### 工作期间

- 默认租约为 45 分钟。长时间工作至少每 15 分钟续租一次：

  ```bash
  scripts/agent-issue-lease.sh heartbeat <issue-number> <agent-id>
  ```

  heartbeat 以远端 lock 为权威：确认 owner 匹配、Issue 仍开放且未显式标记 `agent:blocked` 后，会修复缺失或陈旧的 Issue 状态标签与租约评论。`agent:blocked` 与其他状态标签异常共存时仍具有最高优先级，claim 与 heartbeat 都必须拒绝。不要因为人类可读镜像短暂丢失而手工创建、覆盖或删除 lock 分支。

- 在 Issue 的 `Owned scope` 范围内修改；新增仓库或路径前，先更新 Issue 并确认不与其他活跃 Issue 重叠。
- 每个执行写入的 Agent 必须为当前认领的 Issue 使用独立 Git worktree，并使用独立分支 `agent/issue-<number>-<agent-id>`。不得让多个 Agent 共用同一个 checkout，不得直接在共享主 checkout、其他 Agent 的 worktree、其他 Agent 的分支或含有他人未提交改动的目录中实现功能。
- 共享主 checkout 只用于认领、状态查询、只读检查和创建 worktree；实现、格式化、测试、提交及 push 必须在当前 Agent 自己的 worktree 中完成。创建 worktree 前先确认目标分支名和路径均未被其他 Agent 使用。
- 推荐在认领成功后从最新目标分支创建 worktree：

  ```bash
  git fetch origin main
  git worktree add ../calcit-issue-<number>-<agent-id> -b agent/issue-<number>-<agent-id> origin/main
  ```

  若 Issue 指定其他 base branch，使用 Issue 明确记录的 base；不得自行猜测。一个 Issue 修改多个仓库时，每个被修改仓库都建立独立 worktree，并在 Issue 的 `Owned scope` 中记录路径和分支。
- 一个 worktree 在同一时间只服务一个 Issue 和一个 Agent。切换 Issue、租约转交或恢复旧任务时，不复用仍含另一任务状态的 worktree；先确认原任务已提交、推送并记录，或为新任务创建新的 worktree。
- 不得删除、移动、清理或重置其他 Agent 的 worktree。当前 Agent 的 worktree 只有在关联 PR 已合并或关闭、Issue 已记录最终状态且确认没有未提交或未推送成果后才能清理。
- 发现租约丢失、远端锁所有者改变或 Issue 被关闭时，立即停止写入并保留本地成果，不得强推。
- 只读调查可以并行，但不得借只读任务修改代码、格式化文件或生成构建产物。

### 完成、PR 与租约释放

除“发布流程规范”中明确允许直接提交 `main` 的纯版本号 release commit 外，所有由 Issue 驱动的功能、修复、重构、测试、CI、文档和跨仓库改动，完成验证后都必须推送独立分支并创建关联 PR。仅推送分支、记录 commit URL 或在 Issue 留言都不能替代 PR，也不算进入 review。

- PR 必须关联主 Issue，并在 PR 或 Issue 中记录 PR URL、head commit、base commit、改动仓库与路径、验证命令和结果。
- 一个主 Issue 涉及多个仓库时，每个有改动的仓库都必须创建各自的 PR，并在主 Issue 汇总全部 PR；不能只为其中一个仓库创建 PR。
- GitHub Wiki 仓库没有 PR 审查界面，是上述规则的唯一文档例外：Wiki 修改仍需独立 checkout、独立 commit 并直接 push 到 `calcit.wiki.git`，随后始终在主 Issue 中记录 Wiki commit 与页面 URL；只有任务同时存在主仓库 PR 时，才在该 PR 中重复记录。Wiki 正文不得复制回主仓库规避此例外。
- 已合并或已关闭的 PR 不覆盖其 head 分支后来新增的提交。分支在 PR 合并或关闭后继续产生改动时，必须创建新的关联 PR；不得把旧 PR 当作这些提交已经 review 的证据。
- 只有所需 PR 均已创建、最新提交均已推送且 Issue 已记录上述信息后，才能执行 `release ... review`。没有 PR 时，任务仍可交接则使用 `ready`；确实等待人员权限或外部依赖则使用 `blocked`，并说明分支、commit 和恢复方式。
- 纯版本号 release commit 的直接 `main` 例外只适用于既有发布流程，不适用于夹带功能、修复、测试、CI 或文档改动的提交。

满足以上条件后执行：

```bash
scripts/agent-issue-lease.sh release <issue-number> <agent-id> review
```

若任务仍可由其他 Agent 继续，使用 `ready`；若必须等待人或外部依赖，使用 `blocked`。Agent 异常退出后不需要人工删锁：租约过期时，下一个 `claim` 会用 compare-and-swap 接管，并在 Issue 中更新所有者。查看权威状态使用：

```bash
scripts/agent-issue-lease.sh status <issue-number>
```

### PR 创建后的持续跟进

发出 PR 不代表任务结束。创建 PR 的 Agent 继续负责该 PR，直到合并、关闭或在 Issue 中明确完成交接：

- PR 创建、最新提交推送并记录验证结果后，立即 `release ... review` 释放写入租约。Actions 或 review bot 仍在运行且没有可执行反馈时，不原地等待，可以认领下一个不冲突的 `agent:ready` Issue。
- 一个 Agent 可以维护多个待审 PR 的观察列表，但同一时间只能持有一个 Issue 的写入租约。开始新任务不解除原 PR 的跟进责任。
- 在认领新 Issue 前、完成一个实现检查点后、准备 push 前，以及距上次检查达到 10–15 分钟时，批量检查所有观察中的 PR。不要为单个 pending check 使用紧密轮询，也不使用会阻塞脚本的 `gh run watch`。
- 批量处理 GitHub notifications 时使用唯一的全序：先按“安全风险 → 当前发布阻塞 → required check 新失败 → 其他”的优先级，再按 `updated_at` 升序，最后按数字 notification ID 升序；没有 ID 时用 canonical thread URL 的字典序作为 tie-breaker。前三类越过普通时间顺序时，在关联 Issue 记录原因。
- 通知对应的 Issue 若由其他有效租约持有，只做只读归因，不修改其分支、不把仍需 owner 行动的通知标记已读，随后继续下一个可执行通知。已完成、已失效或已有明确阻塞证据的通知，分类后再标记已读，使未来状态变化可以重新通知。
- 至少使用 `gh pr checks <pr-number>` 检查 Actions，并用 `gh pr view <pr-number> --json state,mergeStateStatus,reviewDecision,statusCheckRollup,reviews,comments` 查看总体状态；inline comments 另外通过 `gh api repos/{owner}/{repo}/pulls/<pr-number>/comments` 检查。
- Actions 失败时先读取失败日志并判断是否由本 PR 引入；能修复的立即修复并重新验证，外部或偶发失败要在 PR 留下证据，不能只重跑后忽略。
- 对每条有效 review 意见进行处理：修改代码或明确回复不修改的理由。不要只回复评论而遗漏对应代码、测试或文档更新。
- Issue 处于 `agent:review` 且出现失败或修改请求时，优先于认领新的普通任务处理。若当前正持有另一个 Issue 的租约，先做到安全检查点并执行 `release ... ready`，再重新 `claim` 待审 Issue；不得同时持有两个写入租约。修复完成、推送并记录验证结果后，再 `release ... review`，随后可恢复之前释放的任务。
- 所有必需 checks 成功且 review 意见已处理后，在 Issue/PR 留下最终状态。若暂时无法继续，标记 `agent:blocked` 并写清阻塞条件和恢复方式。
- PR 合并或关闭后，确认没有残留的 `agent-lock/issue-<number>` 远端锁分支，再关闭或更新主 Issue；涉及关联仓库的 PR 必须逐个记录和确认。

推荐调度优先级为：需要修改的既有 PR → 已认领且未完成的当前 Issue → 新的 `agent:ready` Issue → 仅等待中的 PR。等待中的 PR 只需要周期性批量巡检，不应占用 Agent 的主要执行时间。

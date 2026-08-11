## 开发与验证流程

### 核心步骤

- **代码规范**：执行 `cargo fmt` 保持代码格式一致性。
- **质量检查**：执行 `cargo clippy -- -D warnings` 消除潜在风险与性能问题。
- **构建验证**：执行 `yarn compile` 确保前端 TS 与 Rust 核心构建正常。
- **运行测试**：执行 `cargo test` 验证单元测试，`yarn check-all` 验证全量集成测试。
- **Agent CLI 协议检查**：在本仓库执行 `yarn check-agent-interface`，验证查询命令 stdout 可解析为单个 JSON，并记录语义查询耗时与输出字节数。该命令属于 Calcit 仓库开发流程，不适用于普通业务项目。
- **外部项目回归**：CLI 查询、编辑或类型分析改动完成后，用已全局安装的新 `cr` 在 Respo 等真实项目验证；有写入风险的编辑命令先作用于 Snapshot 临时副本。大项目只需要统计时使用 `--summary-only`，examples 回归优先使用 `check-examples --ns <ns> --def <definition>`。

### 功能准则

- **明确边界**：功能改动需考虑边界条件、错误行为及兼容性。
- **一致性**：复用现有模式，保持日志和错误信息风格统一。
- **测试覆盖**：新功能必须补齐正常路径与异常分支的测试用例。

直接使用命令修改 calcit 程序时不需要调用 cargo, 直接按照文档给出的命令行示例执行即可。

在开始任何 `cr edit` / `cr tree` 修改前，先把下面这条命令当作**硬前置步骤**执行一遍，而不是可选建议：

```bash
cr docs agents --full
```

未先阅读最新 Agent 指南时，不要直接开始改 `calcit.cirru`（兼容旧文件名 `compact.cirru`），避免沿用过时心智模型误判命令边界。

### 运行模式更新（cr / js）

- `cr <entry>`、`cr <entry> js` 默认都是**单次执行**（once）。
- 需要监听时，显式传 `-w` 或 `--watch`（如 `cr -w <entry>`、`cr <entry> js -w`）。
- `cr <entry> ir` 仅用于编译器与生成结果调试，不作为普通项目的运行或验证方式。
- WASM codegen 是仓库内部验证后端，不向用户提供命令；维护者统一通过 `yarn try-wasm` 验证。

### cr eval 基础与常见踩坑

- **用途定位**：`cr eval` 适合快速验证语义/类型提示与宏展开，不等同于完整项目运行。
- **可加载外部模块**：`cr eval` 支持重复传入 `--dep`，可加载多个模块目录（路径以 `/` 结尾时会优先读取其中的 `calcit.cirru`，并回退到 `compact.cirru`）。
  - ✅ `cargo run --bin cr -- calcit/test.cirru eval --dep ~/.config/calcit/modules/respo.calcit/ -- 'ns app.demo $ :require respo.util.detect :refer $ element?\n\nelement? nil'`
- **首表达式 `ns` 会注入当前 eval 程序**：当 snippet 第一个表达式是 `ns` 时，会把 `ns <NS> ...` 从第 3 个节点开始（通常是 `:require` 等规则）合并到运行用的 `ns app.main`，用于在 eval 中显式导入命名空间。
- **`docs check-md` 也支持依赖模块**：`cr docs check-md` 可通过多次 `--dep` 传参，内部会透传给 `eval`/`--check-only`。这样 markdown 代码块可配合首行 `ns ... :require ...` 访问模块函数。
  - ✅ `cargo run --bin cr -- calcit/test.cirru docs check-md docs/CalcitAgent.md --dep ~/.config/calcit/modules/respo.calcit/`
- **顶层无需额外括号**：Cirru 语法本身就不需要"最外层括号"，顶层可以直接是表达式。可用 `cr cirru parse -e` 观察解析结果。
  - ✅ `cargo run --bin cr -- calcit/test.cirru eval 'range 3'`
  - ✅ `cargo run --bin cr -- calcit/test.cirru eval 'let ((x 1)) (+ x 2)'`
  - ❌ `cargo run --bin cr -- calcit/test.cirru eval '(range 3)'`（多一层括号会改变调用语义）
- **`let` 绑定语法**：必须用成对列表，形如 `((name value))`。
  - ✅ `let ((x 1)) x`
  - ❌ `let (x 1) x`（会触发"expects pairs in list for let"）
- **`$ (expr)` 双重求值陷阱**：在 `let` 绑定中，`x $ (f a)` 会被解析为"先求 `(f a)` 再以结果为操作符再调用一次"，触发"cannot be used as operator"。原因是 `$` 后接 `(...)` 形成了两层调用。正确写法是省略 `$`，直接写 `x (f a)` 或 `x f a`。独立一行的 `(expr)` 同理（等同于把结果再调用一次），需加前导 `,` 或改写为非括号形式。
- **末尾符号被当作函数调用**：在 `fn` 或 `let` 的最后一行，单个符号（变量名）会被当作调用（如 `acc` → 触发 "cannot be used as operator"）。需用 `, acc` 加逗号前缀传递值，或将其包在 `println` 等函数调用中返回。
  - ❌ `fn (acc item) if flag (acc) acc`（末尾 `acc` 被当作调用）
  - ✅ `fn (acc item) if flag (acc) , acc`（`, acc` 表示"按值传递"）
- **`foldl` 初始空集合语法**：`foldl xs [] $ fn ...` 中，`[]` 会因 `$` 右结合被解析为 `([] (fn ...))` 而非空列表。正确写法是先绑定 `init $ []`，再传 `init`；或对空 map 同理使用 `init $ {}`。
- **告警会使 eval 失败**：有类型告警时，`cr eval` 会以错误退出（这是预期行为，便于阻断不安全用法）。
  - 例：`cargo run --bin cr -- calcit/test.cirru eval '&list:nth 1 0'` 会提示 `:list` vs `:number` 的类型告警。
- **JS FFI 动态访问告警（opt-in）**：传 `--warn-dyn-method` 时，裸 `JsObject` 上的 `.-`/`.!`/`aget`/`aset`/`js-get`/`js-set`（静态字面量 key）会额外报告 `W_JS_FFI_UNTYPED_ACCESS`，提示声明 external-object trait 提升类型覆盖度；不传 flag 时静默。
- **assert-type 仅做检查**：`assert-type` 在预处理阶段生效，不会改变运行值。
  - 例：`cargo run --bin cr -- calcit/test.cirru eval 'let ((x 1)) (assert-type x :list) x'` 依然返回 `1`，并在检查阶段报告类型不匹配。
- **常用排错方式**：遇到报错先看 `.calcit/error.cirru`，它会提供更完整的栈信息。
- **查示例用法**：可用 `cr query examples <namespace/definition>` 查目标定义的示例。
  - 例：`cargo run --bin cr -- calcit/test.cirru query examples calcit.core/let`

### CLI 修改指南与约束

- **优先使用 `search-replace`**：在 `cr tree` 操作中，优先使用 `search-replace` 而非 `replace`。它基于内容定位，且在不唯一时会报错，比手动指定索引更安全。
- **全量取消 `--stdin` 支持**：由于 Shell 重定向和多行输入的复杂性，所有的修改类子命令（`edit` 和 `tree` 系列）已移除该选项。
  - ✅ 使用 `--code 'code'` 进行单行输入（自动检测 JSON vs Cirru）。
  - ✅ 使用 `--file file` 进行多行或复杂结构输入（推荐在 `.calcit/snippets/` 下创建临时文件）。
  - ✅ 省略 `--code` `--file` 时自动从 stdin 读取（推荐用于多行代码和避免转义）。
- **路径索引动态性**：在 `tree` 系列操作中（如 `delete`, `insert`），操作会引起同级后续节点索引变化。建议**从后往前**操作，或每次修改后使用 `query search` 重新定位。
- **结构引用替换 (`tree rewrite`)**：`tree replace` 仅支持简单替换。涉及引用原始节点及其内容的复杂替换（使用 `--with name=path`）已统一移动至 `cr tree rewrite` 命令。
- **常用的编辑操作**：
  - `edit mv-def <source> <target>`：移动定义到另一命名空间；同命名空间重命名用 `edit rename <source> <new-name>`。
  - `edit cp <target> --from <path> --path <path> [--at <pos>]`：在 AST 节点间复制内容，支持 `before`, `after` (默认), `prepend-child`, `append-child`, `replace`。
- **`cr edit` 命名空间 import 操作格式**（三个命令的 `--code` 输入格式不同，混淆会导致静默损坏）：
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
2. **以 release PR 升级版本**：从已验证的 `main` 创建 `codex/release-<version>` 分支；同步更新 `Cargo.toml`、`package.json`，执行 `cargo update --workspace`，再提交 `chore: release <version>`。版本文件仍通过 release PR 合并，避免在 `main` 上直接制造未审查的版本提交。
3. **准备发布提交**：release PR 合并后拉取最新 `main`，确认版本提交确实位于 `main` HEAD 且目标 tag 尚不存在。可以先检查 Actions 状态，但不要求等待所有 Actions 完成后再继续。
4. **从 main 打 tag 并发布**：允许直接基于合并后的 `main` 提交创建并推送不带 `v` 前缀的 tag，然后创建 GitHub release；打 tag 本身可以在 Actions 尚未完全验证时进行。这一步触发 `publish.yaml` 自动发布到 crates.io 和 npm，之后继续轮询并确认发布 workflow 成功。
5. **最终确认发布成功**：轮询 GitHub Actions 直到 publish workflow 成功，并在 crates.io / npm 上确认新版本可见（版本号一致）。

```bash
# 创建并合并 release PR
git switch -c codex/release-0.13.8 main
# 修改 Cargo.toml、package.json，随后：
cargo update --workspace
git add Cargo.toml package.json Cargo.lock
git commit -m "chore: release 0.13.8"
git push -u origin codex/release-0.13.8
gh pr create --base main --head codex/release-0.13.8 --title "chore: release 0.13.8" --body "..."
gh pr checks <pr-number>
gh pr merge <pr-number> --merge
git switch main
git pull --ff-only origin main

# 打 tag 并推送（不带 v 前缀）
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

- **启动耗时验证**：使用 `time ./target/release/cr calcit/add.cirru`，与改动前同平台 release 基线比较。若数值大幅增加，需检查 `build.rs` 的序列化或 `include_bytes!` 是否失效；旧 `-1` once 参数已经移除。
- **构建体积监控**：使用 `stat -c '%s %n' target/release/cr`（macOS 系统 `stat` 可用等价参数）记录精确字节数，并与改动前同平台 release 基线比较；当前 arm64 产物约 8 MiB，不再使用过时的 5 MiB 绝对阈值。引入新依赖前务必检查其 Transitive Dependencies，优先选择同步轻量库（如 `ureq`）。
- **IO 纯净度检测**：确保 stdout 仅保留程序逻辑输出。版本信息、预热日志、告警均应通过 `eprintln!` 输出至 stderr。验证：`./cr -v > /dev/null` 不应有任何输出。

### 内存性能检查

- **高频分配规约**：在 Preprocessing 阶段，严禁在循环内调用 `Arc::new(CalcitTypeAnnotation::Dynamic)`。应始终 clone 单例 `crate::calcit::DYNAMIC_TYPE`。
- **验证手段**：对于大规模 Cirru 项目的预处理，可通过 `repeat 10 { time ./target/release/cr ... }` 观察耗时抖动。若抖动剧烈，通常预示着堆内存申请频率过高或冷热数据加载策略存在问题。

### JS 代码生成可读性

- JS codegen 采用**行级缩进**（复用 `indent_block`，O(n) 单遍处理），不做 AST 级美化：函数体与嵌套语句块缩进一级，顶层函数间插入空行。改动 codegen 时保持这一低成本风格即可，不要引入完整格式化器。

## 项目结构概览

- `src/`：Rust 核心实现（`src/calcit/` 数据结构, `src/runner/` 运行时, `src/builtins/` 语法, `src/codegen/` IR/JS/WASM 输出）。
- `calcit/`：Cirru 源码与测试用例（`*.cirru`）。
- `lib/` & `js-out/`：JS 共享库与编译输出。
- `docs/` & `demos/`：开发文档与实验性示例。

### 编辑历史

每次 commit 之前, 都产生一个时间戳(年月日时分)开头的文件, 记录本次修改的知识点和概要, 方便未来修改类似功能时查阅.

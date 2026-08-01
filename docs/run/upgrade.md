---
title: "Calcit 项目升级手册（Respo / Lilac）"
summary: "项目升级流程：依赖与工具链同步、entries/type-slots 迁移、静态质量审计、CI 与消费者回归"
scope: "core"
kind: "guide"
category: "run"
aliases:
  - "upgrade"
  - "dependency migration"
  - "respo upgrade"
  - "lilac upgrade"
id: core/run/upgrade
related:
  - core/run/library-quality
  - core/run/entries
  - core/features/static-analysis
---

# Calcit 项目升级手册（Respo / Lilac）

本手册只关注**项目升级流程**，不展开开发实现细节。类库/module 发布前的完整证据矩阵见 [Calcit 类库项目验收与质量门禁](library-quality.md)。

适用对象：通过 Calcit CLI 运行并产出 JS 的项目（例如 Respo）。

---

## 1）升级前检查位置

升级前先检查以下文件与配置是否齐全：

- 运行入口：`calcit.cirru`（兼容旧文件名 `compact.cirru`）
  - `:entries.default`（默认入口及 `:mode`）
  - `:entries.<name>`（额外入口及各自 `:mode`）
- 命令入口：`README`、项目脚本、CI workflow
- Node 工具链：`package.json`、`yarn.lock`、Corepack/Yarn 版本
- 注意 git fetch 检查最新历史, 避免基于老版本操作导致变更冲突
- 结构化编辑优先使用 `cr edit` / `cr tree`；若直接改过 `calcit.cirru`（或旧文件名 `compact.cirru`），提交前执行一次 `cr calcit.cirru edit format`
- 静态质量基线：`check-types`、`weak-types`、公开 namespace examples 与 Markdown 示例

### 快照文件迁移说明

以前的 Calcit 项目使用两套文件：

- `calcit.cirru` — 存放完整 AST 快照，内容包含全部编译信息（带所有代码位置、类型标注等）
- `compact.cirru` — 存放精简代码，是人工读写的主要文件

当前推荐去掉旧的双文件模式，把精简 Snapshot 直接保存在 `calcit.cirru` 中，方便 `cr` 命令直接读取使用。迁移方式：

1. 确认 `compact.cirru` 是项目实际精简化代码，`calcit.cirru` 是完整 AST 快照（差异通常很大）
2. 将 `compact.cirru` 内容直接覆盖到 `calcit.cirru`（或者删除 `calcit.cirru` 后重命名 `compact.cirru` 为 `calcit.cirru`）
3. 提交时删除旧的快照文件（确保不再跟踪 `calcit.cirru` 的旧快照内容）
4. 后续所有 `cr` 命令都基于单一的 `calcit.cirru` 运行，不再生成快照文件

> 如果项目中还存在旧的 `calcit.cirru` 快照文件，迁移后可以删除。`cr` 命令行不再依赖快照文件——它直接读取 `calcit.cirru` 中的精简代码运行。`cr edit` / `cr tree` 的修改直接保存在 `calcit.cirru`。`.gitattributes` 中 `calcit.cirru -diff linguist-generated` 标记也可以移除。`.gitignore` 中的 `compact.cirru` 则应改为忽略旧快照文件名。

---

## 2）标准升级流程（建议顺序）

下面流程按“先确认版本，再对齐工具链，再更新依赖，最后按 CI 链路验证”的顺序执行。

### 快速命令清单

```bash
cr --version
caps upgrade --all
caps
corepack enable
corepack prepare yarn@4.12.0 --activate
yarn install
yarn install --immutable
cr calcit.cirru edit format
cr calcit.cirru --check-only
cr calcit.cirru
yarn vite build --base=./
```

说明：`yarn install` 只在 lockfile 迁移或依赖变更时需要；平时可直接从 `yarn install --immutable` 开始。

### Step A：确认 Calcit CLI 版本

```bash
cr --version
```

说明：一般本机已经是较新版本，但升级前先确认一遍，避免后续误判。

> ⚠️ 注意 CI 中 `calcit-lang/setup-cr` 的版本：旧版本（如 `0.0.8`）安装的 `cr` 可能不识别 `calcit.cirru`。升级后若 CI 报 `"compact.cirru does not exist"`，请升级 `setup-cr` 版本。最新版本请查阅 [setup-cr releases](https://github.com/calcit-lang/setup-cr/releases)。

### Step B：先对齐项目版本与 Node 工具链

重点先检查并对齐以下几处：

- `deps.cirru` 里的 `:calcit-version`
- `package.json` 里的 `@calcit/procs`
- `package.json` 里的 `packageManager`
- `.yarnrc.yml` 是否需要 `nodeLinker: node-modules`
- `.gitignore` 是否已忽略 `.yarn/*.gz`（避免 Yarn 压缩状态文件入库）

先把这些基础版本与工具链约定对齐，再继续更新依赖，能减少后面重复改 lockfile 或 CI 的次数。

### Step C：检查并更新 `deps.cirru`

```bash
caps upgrade --all
```

说明：`caps upgrade --all` 会更新 `deps.cirru` 里的依赖版本与 `:calcit-version`；如果确实发生升级，还会顺带执行一次 `yarn up @calcit/procs`，把 JS 运行时包同步到当前 Calcit 版本链路。

如果你只想批量把旧版本提升到最新标签，也可以继续用：

```bash
caps outdated --yes
```

这个命令只更新 `deps.cirru`，不触发 `yarn up @calcit/procs`。

### Step D：同步模块内容

```bash
caps
```

说明：这一步才会按当前 `deps.cirru` 下载/同步模块内容。

### Step E：用 Yarn Berry 安装并校验

```bash
corepack enable
corepack prepare yarn@4.12.0 --activate
yarn --version
yarn install --immutable
```

说明：团队若习惯 Yarn Berry，建议固定 `packageManager` 并使用 `--immutable` 做一致性校验。

如果项目仍依赖 `node_modules` 目录解析，还应补一个 `.yarnrc.yml`：

```yaml
nodeLinker: node-modules
```

### Step F：从 CI workflow 和 package.json 提取检查命令并本地先跑

先看 `.github/workflows/` 里实际执行了哪些命令，再看 `package.json` 里是否有额外构建脚本，然后按同顺序在本地跑一遍。

常见链路例如：

```bash
caps && yarn install --immutable
cr calcit.cirru --check-only
cr calcit.cirru
cr calcit.cirru --entry <entry-name>
cr calcit.cirru js && yarn vite build --base=./
```

`cr calcit.cirru` 默认单次执行并选择 `entries.default`；指定 entry 时用 `--entry <name>`。entry 的 `:mode` 已决定 native 运行或 JS 生成，项目脚本与 CI 应优先依赖该配置。只有热更新才加 `-w` / `--watch`；显式 `js` 是兼容/定向 codegen 覆盖，不是每个 JS 项目的必需写法。

`--check-only` 会同时预处理所选 entry 的 init/reload 定义，因此可以发现遗留的 `:reload-fn` 已不存在等旧配置问题。

如果 `package.json` 里有编译、构建、测试相关脚本，也应本地执行一遍；没有额外脚本可跳过。若项目直接通过 Vite 构建，可执行：

```bash
yarn up vite
yarn vite build --base=./
```

说明：若项目依赖 Vite，升级时建议显式执行一次 `yarn up vite`，并重跑构建确认兼容性。

例如还有：

```bash
yarn <script-name>
```

目标：把 CI 会跑的命令和项目脚本都在本地提前验证，减少合并后失败概率。

---

## 3）近期项目结构与类型迁移

### 3.1 统一 entries

顶层 `:configs` 是兼容输入，不应继续作为新配置写法。执行：

```bash
cr calcit.cirru edit format
cr calcit.cirru config show
```

format 会把旧 `:configs` 迁移为 `:entries.default`，并输出 `W_LEGACY_CONFIG`。随后应审阅：

- 每个 entry 都有明确的 `:mode :native` 或 `:mode :js`。
- named entry 是完整配置，不继承 default 的 modules/type slots。
- `:init-fn` / `:reload-fn` 使用 definition symbol，而不是继续新增字符串值。
- entry 用 `:description` 说明用途，便于维护者和 Agent 选择正确入口。

### 3.2 Type slots

类库用 `deftype-slot` 声明由应用提供的编译期类型时，每个使用该 slot 的 entry 都要单独绑定：

```bash
cr calcit.cirru config type-slots
cr calcit.cirru config set-type-slot :dispatch-op app.schema/DispatchOp
cr calcit.cirru config set-type-slot --entry test :dispatch-op app.test-schema/TestDispatchOp
```

未绑定 slot 会回退到 `:dynamic`，可能让 callback 检查和 method specialization 失去静态证据。升级后应逐 entry 检查，而不是只验证 default。

### 3.3 旧 schema 与动态类型

`:any` 只保留为 `:dynamic` 的兼容拼写；新 schema 不应继续引入。`edit format` 会输出 `W_LEGACY_ANY` / `W_DYNAMIC_TYPE_DEBT`，但不会猜测并自动改写类型关系。执行：

```bash
cr calcit.cirru analyze check-types --summary-only
cr calcit.cirru analyze weak-types \
  --only schema-dynamic,code-dynamic \
  --intent unresolved \
  --summary-only
```

有命中时去掉 `--summary-only` 查看路径和建议。输入/输出共享类型时用 `:generics`，只约束能力时用 trait `:where`，collection/ref 保留类型参数，有限异构值使用 enum。真正的 JS FFI 边界应显式标记 `:features $ #{} :js-ffi`，并在进入 typed code 前 validate/convert。

### 3.4 format 的边界

`cr edit format` 负责可解析性、canonical serialization 和已知旧结构迁移；它不是完整的语义 linter。告警写到 stderr 且不会阻止格式化。CI 需要在 format 后检查 `git diff`，并单独读取 `check-types` / `weak-types --format json` 来执行项目自己的质量阈值。

---

## 4）Yarn Berry 升级检查

### 4.1 packageManager 固定

```json
{
  "packageManager": "yarn@4.12.0"
}
```

### 4.2 CI 基础模板（GitHub Actions）

```yaml
- uses: actions/setup-node@v6
  with:
    node-version: 24

- name: Enable Corepack
  run: |
    corepack enable
    corepack prepare yarn@4.12.0 --activate
    yarn --version

- uses: calcit-lang/setup-cr@0.0.9

- name: Install deps
  run: caps --ci && yarn install --immutable

- name: Validate Calcit snapshot and types
  run: |
    cr calcit.cirru edit format
    git diff --exit-code -- calcit.cirru
    cr calcit.cirru --check-only
    cr calcit.cirru analyze check-types --summary-only --format json
    cr calcit.cirru analyze weak-types --only schema-dynamic,code-dynamic --intent unresolved --summary-only --format json
```

> ⚠️ `calcit-lang/setup-cr` 的版本决定了 CI 中安装的 `cr` 版本。旧版本（如 `0.0.8`）可能不识别 `calcit.cirru`，且不支持新版类型检查。建议保持 `@0.0.9` 或更新。最新版本请查阅 [setup-cr releases](https://github.com/calcit-lang/setup-cr/releases)。不要在 `caps --ci` 之前运行 `cr` 命令，否则会使用默认旧版模块缓存。

说明：若项目依赖 `packageManager: "yarn@4.12.0"`，优先先执行 Corepack 激活，再让 CI 触发 Yarn。不要让 `setup-node` 的 Yarn cache 或其他 Yarn 调用早于 `corepack enable` / `corepack prepare`，否则可能误用 runner 上的全局 Yarn 1。 `caps --ci` 参数保证在 CI 加载模块时使用 HTTPS 协议，避免 CI 环境下的 SSH key 问题。

注意：两个 analysis 命令会报告诊断，但不会替项目决定 debt 阈值。严格 CI 应解析 JSON summary，与零目标或已审阅 baseline 比较。

### 4.3 lockfile 迁移

如果 `yarn install --immutable` 因 lockfile 格式变化失败：

1. 先执行一次 `yarn install` 生成新格式 lockfile；
2. 再执行 `yarn install --immutable` 做严格校验。

---

## 5）升级后最小验证矩阵

建议至少覆盖以下项目：

1. `cr --version`
2. `caps upgrade --all`（确认无遗漏项或已按预期处理）
3. `yarn install --immutable`
4. `cr calcit.cirru edit format` 后 `git diff --exit-code -- calcit.cirru`
5. `cr calcit.cirru --check-only`
6. 所有声明支持的 entry（默认 once；watch 另行验收）
7. `check-types` 与 unresolved `weak-types` 基线
8. 公开 namespace 的 `check-examples` 与 `docs check-md`
9. JS 项目的 codegen 加 Node/Vite 行为测试，而不只是生成成功
10. `package.json` 中与编译/构建相关的脚本
11. 类库项目在 Respo 等真实消费者中的回归证据

call graph 的 `--show-unused` 只能作为 entry-relative 线索；公开 API 和替代入口可能被列为 unreachable，不能据此自动删除。

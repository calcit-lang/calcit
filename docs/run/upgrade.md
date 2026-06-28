---
title: "Calcit 项目升级手册（Respo / Lilac）"
summary: "项目升级流程：caps upgrade、deps.cirru 版本同步、CI workflow 更新、Corepack/Yarn 配置迁移"
scope: "core"
kind: "guide"
category: "run"
aliases:
  - "upgrade"
  - "dependency migration"
  - "respo upgrade"
  - "lilac upgrade"
---

# Calcit 项目升级手册（Respo / Lilac）

本手册只关注**项目升级流程**，不展开开发实现细节。

适用对象：通过 Calcit CLI 运行并产出 JS 的项目（例如 Respo）。

---

## 1）升级前检查位置

升级前先检查以下文件与配置是否齐全：

- 运行入口：`calcit.cirru`（兼容旧文件名 `compact.cirru`）
  - `:configs`（默认入口）
  - `:entries`（额外入口）
- 命令入口：`README`、项目脚本、CI workflow
- Node 工具链：`package.json`、`yarn.lock`、Corepack/Yarn 版本
- 注意 git fetch 检查最新历史, 避免基于老版本操作导致变更冲突
- 结构化编辑优先使用 `cr edit` / `cr tree`；若直接改过 `calcit.cirru`（或旧文件名 `compact.cirru`），提交前执行一次 `cr edit format`

### 快照文件迁移说明

以前的 Calcit 项目使用两套文件：

- `calcit.cirru` — 存放完整 AST 快照，内容包含全部编译信息（带所有代码位置、类型标注等）
- `compact.cirru` — 存放精简代码，是人工读写的主要文件

当前推荐去掉快照文件，精简代码直接保存在 `calcit.cirru` 中，方便 `cr` 命令直接读取使用。迁移方式：

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
cr js
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
cr --entry <entry-name>
cr --entry <entry-name> js
cr js && yarn vite build --base=./
```

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

## 3）Yarn Berry 升级检查

### 3.1 packageManager 固定

```json
{
  "packageManager": "yarn@4.12.0"
}
```

### 3.2 CI 基础模板（GitHub Actions）

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
```

> ⚠️ `calcit-lang/setup-cr` 的版本决定了 CI 中安装的 `cr` 版本。旧版本（如 `0.0.8`）可能不识别 `calcit.cirru`，且不支持新版类型检查。建议保持 `@0.0.9` 或更新。最新版本请查阅 [setup-cr releases](https://github.com/calcit-lang/setup-cr/releases)。不要在 `caps --ci` 之前运行 `cr` 命令，否则会使用默认旧版模块缓存。

说明：若项目依赖 `packageManager: "yarn@4.12.0"`，优先先执行 Corepack 激活，再让 CI 触发 Yarn。不要让 `setup-node` 的 Yarn cache 或其他 Yarn 调用早于 `corepack enable` / `corepack prepare`，否则可能误用 runner 上的全局 Yarn 1。 `caps --ci` 参数保证在 CI 加载模块时使用 HTTPS 协议，避免 CI 环境下的 SSH key 问题。

### 3.3 lockfile 迁移

如果 `yarn install --immutable` 因 lockfile 格式变化失败：

1. 先执行一次 `yarn install` 生成新格式 lockfile；
2. 再执行 `yarn install --immutable` 做严格校验。

---

## 4）升级后最小验证矩阵

建议至少覆盖以下 6 项：

1. `cr --version`
2. `caps upgrade --all`（确认无遗漏项或已按预期处理）
3. `yarn install --immutable`
4. `cr js`（如果是 js 项目）
5. CI 中的入口/测试命令（`--entry` 或 `--init-fn` 链路）
6. `package.json` 中与编译/构建相关脚本，或直接执行 `yarn vite build --base=./`

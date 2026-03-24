# Calcit 项目升级手册（Respo / Lilac）

本手册只关注**项目升级流程**，不展开开发实现细节。

适用对象：通过 Calcit CLI 运行并产出 JS 的项目（例如 Respo）。

---

## 1）升级前检查位置

升级前先检查以下文件与配置是否齐全：

- 运行入口与快照：`compact.cirru`
  - `:configs`（默认入口）
  - `:entries`（额外入口）
- 命令入口：`README`、项目脚本、CI workflow
- Node 工具链：`package.json`、`yarn.lock`、Corepack/Yarn 版本
- 注意 git fetch 检查最新历史, 避免基于老版本操作导致变更冲突

---

## 2）标准升级流程（建议顺序）

下面流程按“先确认版本，再对齐工具链，再更新依赖，最后按 CI 链路验证”的顺序执行。

### Step A：确认 Calcit CLI 版本

```bash
cr --version
```

说明：一般本机已经是较新版本，但升级前先确认一遍，避免后续误判。

### Step B：先对齐项目版本与 Node 工具链

重点先检查并对齐以下几处：

- `deps.cirru` 里的 `:calcit-version`
- `package.json` 里的 `@calcit/procs`
- `package.json` 里的 `packageManager`
- `.yarnrc.yml` 是否需要 `nodeLinker: node-modules`
- `.gitignore` 是否已忽略 `.yarn/*.gz`，避免 Yarn 生成的压缩状态文件入库

先把这些基础版本与工具链约定对齐，再继续更新依赖，能减少后面重复改 lockfile 或 CI 的次数。

### Step C：检查并更新 `deps.cirru`

```bash
caps outdated --yes
```

说明：

- `caps outdated`：查看可更新项；
- `caps outdated --yes`：直接更新 `deps.cirru`（无交互确认）。
- `caps`：根据当前 `deps.cirru` 下载/同步模块内容。

注意：`caps outdated --yes` 只负责更新 `deps.cirru`，不等于已经完成模块同步。

若依赖是固定 tag/version，仍需先改 `deps.cirru` 再执行更新。

### Step D：同步模块内容

```bash
caps
```

说明：这一步才是根据当前 `deps.cirru` 下载/同步模块内容。若跳过这一步，后面的编译与安装结果容易混入旧模块状态。

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

如果 `package.json` 里有与编译、构建、测试相关的脚本，也应本地执行一遍；如果没有额外脚本，这一步可以跳过。若项目直接通过 Vite 构建，也可直接执行：

```bash
yarn up vite
yarn vite build --base=./
```

说明：最近 Vite 有大版本更新。若项目依赖 Vite，升级时建议显式执行一次 `yarn up vite`，并在更新后重新跑 `yarn vite build --base=./` 确认没有新的构建兼容性问题。

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

- name: Install deps
  run: yarn install --immutable
```

说明：若项目依赖 `packageManager: "yarn@4.12.0"`，优先先执行 Corepack 激活，再让 CI 触发 Yarn。不要让 `setup-node` 的 Yarn cache 或其他 Yarn 调用早于 `corepack enable` / `corepack prepare`，否则可能误用 runner 上的全局 Yarn 1。

### 3.3 lockfile 迁移

如果 `yarn install --immutable` 因 lockfile 格式变化失败：

1. 先执行一次 `yarn install` 生成新格式 lockfile；
2. 再执行 `yarn install --immutable` 做严格校验。

---

## 4）升级后最小验证矩阵

建议至少覆盖以下 6 项：

1. `cr --version`
2. `caps outdated --yes`（确认无遗漏项或已按预期处理）
3. `yarn install --immutable`
4. `cr js`（如果是 js 项目）
5. CI 中的入口/测试命令（`--entry` 或 `--init-fn` 链路）
6. `package.json` 中与编译/构建相关脚本，或直接执行 `yarn vite build --base=./`

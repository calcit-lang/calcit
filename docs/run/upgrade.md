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

---

## 2）标准升级流程（建议顺序）

下面流程按“先确认版本，再更新依赖，再验证命令链”的顺序执行。

### Step A：确认 Calcit CLI 版本

```bash
cr --version
```

说明：一般本机已经是较新版本，但升级前先确认一遍，避免后续误判。

### Step B：检查项目内 Calcit 版本对齐

重点检查两处是否一致、是否为目标版本：

- `deps.cirru` 里的 `:calcit-version`
- `package.json` 里的 `@calcit/procs`

必要时同步更新这两处，避免运行时和 JS 依赖版本错位。

### Step C：检查并更新依赖

```bash
caps outdated --yes
```

说明：

- `caps outdated`：查看可更新项；
- `caps outdated --yes`：直接更新 `deps.cirru`（无交互确认）。

若依赖是固定 tag/version，仍需先改 `deps.cirru` 再执行更新。

### Step D：用 Yarn Berry 安装并校验

```bash
corepack enable
corepack prepare yarn@4.12.0 --activate
yarn --version
yarn install --immutable
```

说明：团队若习惯 Yarn Berry，建议固定 `packageManager` 并使用 `--immutable` 做一致性校验。

### Step E：从 CI workflow 提取检查命令并本地先跑

先看 `.github/workflows/` 里实际执行了哪些命令，然后按同顺序在本地跑一遍。

常见链路例如：

```bash
caps --ci && yarn install --immutable
cr --entry <entry-name>
cr --entry <entry-name> js
cr js && yarn vite build --base=./
```

### Step F：执行 package.json 里的编译相关脚本

如果 `package.json` 里有与编译、构建、测试相关的脚本，也应本地执行一遍，确认升级后仍可用。

例如：

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

### 3.3 lockfile 迁移

如果 `yarn install --immutable` 因 lockfile 格式变化失败：

1. 先执行一次 `yarn install` 生成新格式 lockfile；
2. 再执行 `yarn install --immutable` 做严格校验。

---

## 4）升级后最小验证矩阵

建议至少覆盖以下 6 项：

1. `cr --version`
2. `caps --ci outdated`（确认无遗漏项或已按预期处理）
3. `yarn install --immutable`
4. `cr js`(如果是 js 项目)
5. CI 中的入口/测试命令（`--entry` 或 `--init-fn` 链路）
6. `package.json` 中与编译/构建相关脚本

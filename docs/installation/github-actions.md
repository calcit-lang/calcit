---
title: "GitHub Actions"
summary: "使用 deps.cirru 固定的 Calcit 版本安装工具链，并运行严格检查与目标后端测试。"
scope: "core"
kind: "reference"
category: "installation"
aliases:
  - "github actions"
  - "ci"
  - "workflow"
  - "setup-cr"
  - "setup-calcit"
  - "Calcit CI"
id: core/installation/github-actions
related:
  - core/run/library-quality
  - core/features/static-analysis
entry_for:
  - "setup-cr"
  - "setup-calcit"
  - "Calcit GitHub Actions"
---

# GitHub Actions

For a normal Calcit project, declare the compiler version once in `deps.cirru`:

```cirru.no-check
{} $ :calcit-version |0.13.27
```

Then install it after checkout. Do not repeat the Calcit version in the workflow:

```yaml
- uses: actions/checkout@v4

- uses: actions/setup-node@v6
  with:
    node-version: 24

- name: Enable Yarn
  run: corepack enable && corepack prepare yarn@4.12.0 --activate

- uses: calcit-lang/setup-calcit@v1
```

`setup-calcit` reads the selected `deps.cirru` when no explicit version input is supplied. A missing selected
file is treated as a task without a project declaration, so it requires an explicit version; a file with
no `:calcit-version` behaves the same way. Malformed or duplicate declarations fail rather than falling
back to `version`. Do not provide two version sources for a regular project. The Action release controls
installer behavior, while `:calcit-version` controls the Calcit runtime/compiler release. Caps has an independent
release version; setup-calcit pins a verified stable default and exposes `caps-version` when a workflow needs an
explicit package-manager pin. Version 1 adds `cr -> calcit` inside the Action tool directory so an existing `run: cr ...` command keeps working
during workflow migration. For pre-rename releases it falls back to their `cr` asset and exposes `calcit`; new
and edited commands should use `calcit`.

Existing `calcit-lang/setup-cr` workflows remain supported by their published tags. GitHub Actions does
not follow action-repository rename redirects, so use `setup-calcit` for new workflows and migrate an
old workflow only by intentionally replacing its `uses:` reference.

Then to load packages defined in `deps.cirru` with `caps`:

```bash
caps --ci
```

JS 项目先安装锁定的 runtime 依赖，再运行与本地一致的严格检查：

```yaml
- name: Install dependencies
  run: caps --ci && yarn install --immutable

- name: Verify Calcit runtime toolchain
  run: caps verify --toolchain

- name: Validate Snapshot and type quality
  run: |
    calcit calcit.cirru edit format
    git diff --exit-code -- calcit.cirru
    calcit calcit.cirru --check-only
```

以上步骤只覆盖安装与静态层。生成 JavaScript 的项目还必须增加目标 runtime 命令，例如 Node 项目另行编译并执行 smoke/contract test：

```yaml
- name: Run generated Node runtime test
  run: yarn run:node
```

Browser 项目应运行等价的 headless-browser test；只有 codegen 成功不能证明 runtime 契约。

新项目不要生成 quality baseline。`check-types` 与 `weak-types` 只用于迁移定位；类型正确性由默认严格预处理的 warning/error 决定。已有项目可在 0.14.x 暂时保留 `analyze quality --baseline ...`，但只允许降低预算，清零后应删除。完整的 entry、example、后端测试与消费者回归要求见 [Calcit 类库项目验收与质量门禁](../run/library-quality.md)。

The JavaScript runtime dependency remains in `package.json`/its lockfile. Keep it compatible with the
Calcit release declared by the project, and execute generated JS in CI; a successful codegen alone does
not verify host imports or runtime proc compatibility. For typed host bindings, read
[JavaScript Interop](../features/js-interop.md).

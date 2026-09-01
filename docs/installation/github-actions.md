---
title: "GitHub Actions"
summary: "Use setup-calcit with the project version in deps.cirru, then run caps and the native quality gate."
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

For a JS project, install its locked runtime dependencies and run the same native quality gate used
locally:

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
    calcit calcit.cirru analyze quality --baseline config/calcit-quality.cirru
```

The workflow above covers installation and the static layer. A project that emits JavaScript must add
its target-specific runtime command; for example, a Node project can compile and execute its smoke or
contract test as a separate step:

```yaml
- name: Run generated Node runtime test
  run: yarn run:node
```

Browser projects should run an equivalent headless-browser test. Code generation alone is not runtime
evidence.

New libraries without existing debt can omit `--baseline` and use the zero-debt gate. `check-types`
and `weak-types` are reports for diagnosis; `analyze quality` is the CI command that fails on a
regression. See [Calcit 类库项目验收与质量门禁](../run/library-quality.md) for entries, examples,
backend tests, and consumer regression requirements.

The JavaScript runtime dependency remains in `package.json`/its lockfile. Keep it compatible with the
Calcit release declared by the project, and execute generated JS in CI; a successful codegen alone does
not verify host imports or runtime proc compatibility. For typed host bindings, read
[JavaScript Interop](../features/js-interop.md).

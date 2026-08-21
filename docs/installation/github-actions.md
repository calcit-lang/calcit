---
title: "GitHub Actions"
summary: "Use setup-cr with the project version in deps.cirru, then run caps and the native quality gate."
scope: "core"
kind: "reference"
category: "installation"
aliases:
  - "github actions"
  - "ci"
  - "workflow"
  - "setup-cr"
  - "Calcit CI"
id: core/installation/github-actions
related:
  - core/run/library-quality
  - core/features/static-analysis
entry_for:
  - "setup-cr"
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

- uses: calcit-lang/setup-cr@0.0.9
```

`setup-cr` reads `deps.cirru` when no explicit version input is supplied. Use an explicit version only
for a task that intentionally has no project `deps.cirru`; do not provide two version sources for a
regular project. The Action release controls installer behavior, while `:calcit-version` controls which
`cr` and `caps` release the project uses.

Then to load packages defined in `deps.cirru` with `caps`:

```bash
caps --ci
```

For a JS project, install its locked runtime dependencies and run the same native quality gate used
locally:

```yaml
- name: Install dependencies
  run: caps --ci && yarn install --immutable

- name: Validate Snapshot and type quality
  run: |
    cr calcit.cirru edit format
    git diff --exit-code -- calcit.cirru
    cr calcit.cirru --check-only
    cr calcit.cirru analyze quality --baseline config/calcit-quality.json
```

New libraries without existing debt can omit `--baseline` and use the zero-debt gate. `check-types`
and `weak-types` are reports for diagnosis; `analyze quality` is the CI command that fails on a
regression. See [Calcit 类库项目验收与质量门禁](../run/library-quality.md) for entries, examples,
backend tests, and consumer regression requirements.

The JavaScript runtime dependency remains in `package.json`/its lockfile. Keep it compatible with the
Calcit release declared by the project, and execute generated JS in CI; a successful codegen alone does
not verify host imports or runtime proc compatibility. For typed host bindings, read
[JavaScript Interop](../features/js-interop.md).

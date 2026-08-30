---
title: "Run Calcit"
scope: "core"
kind: "hub"
category: "run"
aliases:
  - "run calcit"
  - "watch mode"
  - "entry file"
  - "hot reload"
entry_for:
  - "calcit"
  - "calcit js"
id: core/run
---
# Run Calcit

This page is a quick navigation hub. Detailed topics are split into dedicated chapters under `run/`.

## Quick start

Run local project once (default behavior):

```bash
calcit
```

Enable watch mode explicitly:

```bash
calcit -w
```

Evaluate a snippet:

```bash
calcit eval 'println "|Hello world"'
```

Emit JavaScript once:

```bash
calcit js
```

## Run guide map

- [Run in Eval mode](./run/eval.md)
- [CLI Options](./run/cli-options.md)
- [Development Debugging](./run/debugging.md)
- [Querying definitions](./run/query.md)
- [Experimental Calx target eligibility](./run/calx-target.md)
- [Calcit→Calx benchmark methodology](./run/calx-benchmark.md)
- [Documentation & Libraries](./run/docs-libs.md)
- [CLI Code Editing](./run/edit-tree.md)
- [Load Deps](./run/load-deps.md)
- [Hot Swapping](./run/hot-swapping.md)
- [Bundle Mode](./run/bundle-mode.md)
- [Entries](./run/entries.md)
- [Project Upgrade Playbook](./run/upgrade.md)

## Quick find by keyword

Use these keywords directly with `calcit docs read` for faster section hits:

- `eval`, `snippet`, `dep`, `type-check` → [Run in Eval mode](./run/eval.md)
- `watch`, `once`, `entry`, `reload-fn` → [CLI Options](./run/cli-options.md)
- `query`, `find`, `usages`, `search-expr` → [Querying definitions](./run/query.md)
- `docs`, `read-lines`, `libs`, `readme` → [Documentation & Libraries](./run/docs-libs.md)
- `edit`, `tree`, `search-replace`, `imports` → [CLI Code Editing](./run/edit-tree.md)

Typical navigation flow:

```bash
# 1) List headings in a chapter
calcit docs read run.md

# 2) Jump by keyword(s)
calcit docs read run.md quick find

# 3) Open the target chapter and narrow again
calcit docs read query.md usages
```

Use this page for orientation, then jump to the specific chapter for complete examples and edge cases.

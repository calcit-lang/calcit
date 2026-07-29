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
  - "cr"
  - "cr js"
id: core/run
---
# Run Calcit

This page is a quick navigation hub. Detailed topics are split into dedicated chapters under `run/`.

## Quick start

Run local project once (default behavior):

```bash
cr
```

Enable watch mode explicitly:

```bash
cr -w
```

Evaluate a snippet:

```bash
cr eval 'println "|Hello world"'
```

Emit JavaScript once:

```bash
cr js
```

## Run guide map

- [Run in Eval mode](./run/eval.md)
- [CLI Options](./run/cli-options.md)
- [Development Debugging](./run/debugging.md)
- [Querying definitions](./run/query.md)
- [Documentation & Libraries](./run/docs-libs.md)
- [CLI Code Editing](./run/edit-tree.md)
- [Load Deps](./run/load-deps.md)
- [Hot Swapping](./run/hot-swapping.md)
- [Bundle Mode](./run/bundle-mode.md)
- [Entries](./run/entries.md)
- [Project Upgrade Playbook](./run/upgrade.md)

## Quick find by keyword

Use these keywords directly with `cr docs read` for faster section hits:

- `eval`, `snippet`, `dep`, `type-check` → [Run in Eval mode](./run/eval.md)
- `watch`, `once`, `entry`, `reload-fn` → [CLI Options](./run/cli-options.md)
- `query`, `find`, `usages`, `search-expr` → [Querying definitions](./run/query.md)
- `docs`, `read-lines`, `libs`, `readme` → [Documentation & Libraries](./run/docs-libs.md)
- `edit`, `tree`, `search-replace`, `imports` → [CLI Code Editing](./run/edit-tree.md)

Typical navigation flow:

```bash
# 1) List headings in a chapter
cr docs read run.md

# 2) Jump by keyword(s)
cr docs read run.md quick find

# 3) Open the target chapter and narrow again
cr docs read query.md usages
```

Use this page for orientation, then jump to the specific chapter for complete examples and edge cases.

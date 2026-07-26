---
title: "CLI Code Editing (edit & tree)"
summary: "如何使用 cr tree show/replace/search-replace/delete/batch-delete/insert/wrap/rewrite 查看和修改 AST 节点"
scope: "core"
kind: "reference"
category: "run"
aliases:
  - "edit tree"
  - "search-replace"
  - "add-import"
  - "tree replace"
  - "tree rewrite"
entry_for:
  - "cr tree search-replace"
  - "cr edit add-import"
  - "cr query search"
id: core/run/edit-tree
parent: core/run
related:
  - core/run/query
  - core/structural-editor
requires:
  - core/run/query
leads_to:
  - core/structural-editor
---

# CLI Code Editing (edit & tree)

Calcit provides powerful CLI tools for modifying code directly without opening a text editor. These commands are optimized for both interactive use and automated scripts/agents.

## Core Editing (cr edit)

The `edit` command handles high-level operations on namespaces and definitions.

```bash
# Refresh snapshot formatting without semantic changes
cr edit format
```

This command also rewrites older namespace records into the canonical `NsEntry` snapshot shape.

### Managing Namespaces

```bash
# Move a definition to another namespace
cr edit mv-def app.main/old-name app.util/old-name

# Rename a definition within the same namespace
cr edit rename app.main/old-name new-name

# Add a new namespace
cr edit add-ns app.util

# Remove a namespace
cr edit rm-ns app.util
```

### Managing Imports

```bash
# Add an import to a namespace
cr edit add-import app.main --code 'quote (respo.core :refer $ deftime)'

# Bulk reset all imports for a namespace
cr edit imports app.main --file imports.cirru
```

### Managing Schemas and Examples

```bash
# Schema accepts exactly one quoted Cirru type node.
cr edit schema 'app.main/*enabled?' --code 'quote $ :: :ref :bool'

# Each top-level quote becomes one example; leaves remain representable.
cr edit examples app.main/add << 'END'
quote $ add 1 2
quote $ add 3 4
quote |literal
END

# Execute only the examples attached to one definition.
cr analyze check-examples --ns app.main --def add
```

## Fine-grained AST Operations (cr tree)

The `tree` command allows precise manipulation of nodes within a definition's S-expression tree.

### Viewing the Tree

```bash
# View the AST of a definition with indices
cr tree show app.main/main!
```

### Target-based Replacement

`search-replace` is the safest way to modify a specific node by its content:

```bash
# Replace '1' with '10' inside the definition
cr tree search-replace app.main/main! --pattern '1' --code 'quote |10'
```

### Path-based Operations

You can use numeric paths to locate deep nodes:

```bash
# Replace the node at path @1.2.0
cr tree replace app.main/main! --path '@1.2.0' --code 'quote ((+ 1 2))'

# Insert before a node
cr tree insert-before app.main/main! --path '@1.0' --code 'quote (println |started)'

# Delete a node
cr tree delete app.main/main! --path '@1.0'
```

### Copying and Moving Nodes

Current CLI exposes node copy/move under `cr edit cp` and `cr edit mv`:

```bash
# Copy a node within a definition
cr edit cp app.main/target-def --from '@1.0' --path '@2.0' --at append-child

# Move a node within a definition
cr edit mv app.main/target-def --from '@1.0' --path '@2.0' --at after
```

## Input Formats

Editing commands support several ways to provide new code:

- `--code 'code'`: Inline text (auto-detects JSON vs Cirru format).
- `--file file.cirru`: Multi-line code from a file (recommended for complex structures).
- **stdin**: Pipe or redirect input directly; auto-detects JSON vs Cirru.

For Cirru input, current CLI expects **Cirru EDN with `quote` prefix**:

- `--code 'quote |leaf'`
- `--code 'quote (expr ...)'`
- stdin / `--file` likewise use `quote ...`
- only JSON array input can be passed without `quote`

`cr edit schema` follows the same rule and accepts exactly one quoted type node. `cr edit examples` is the batch form: each top-level item must independently be `quote |leaf` or `quote $ expr ...`. A quoted `[]` wrapper is not a batch marker because it would describe one AST node, not a collection of edit operations.

> Note: For multi-line text input, prefer `--file` or stdin heredoc. They avoid shell escaping, but Cirru content still needs the `quote` prefix.

## Best Practices

1. **Check first**: Use `cr query find` or `cr tree show` to confirm the current state.
2. **From back to front**: When performing multiple `delete` or `insert` operations at the same level, start from the highest index to avoid shifting indices.
3. **Use search-replace**: It is usually safer than path-based replacement as it validates the current content.

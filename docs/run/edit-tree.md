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
# Move or rename a definition
cr edit mv app.main/old-name app.main/new-name

# Add a new namespace
cr edit add-ns app.util

# Remove a namespace
cr edit rm-ns app.util
```

### Managing Imports

```bash
# Add an import to a namespace
cr edit add-import app.main -e 'respo.core :refer $ deftime'

# Bulk reset all imports for a namespace
cr edit imports app.main -f imports.cirru
```

## Fine-grained AST Operations (cr tree)

The `tree` command allows precise manipulation of nodes within a definition's S-expression tree.

### Viewing the Tree

```bash
# View the AST of a definition with indices
cr tree view app.main/main!
```

### Target-based Replacement

`target-replace` is the safest way to modify a specific node by its content:

```bash
# Replace '1' with '10' inside the definition
cr tree target-replace app.main/main! -t 1 -e 10
```

### Path-based Operations

You can use numeric paths to locate deep nodes:

```bash
# Replace the node at path [1 2 0]
cr tree replace app.main/main! -p 1 2 0 -e '(+ 1 2)'

# Insert before/after a node
cr tree insert app.main/main! -p 1 0 --at before -e 'println |started'

# Delete a node
cr tree delete app.main/main! -p 1 0
```

### Copying across Definitions

```bash
# Copy a node from one definition to another
cr tree cp app.main/target-def --from app.main/source-def -p 1 0 --at append-child
```

## Input Formats

Editing commands support several ways to provide new code:

- `-e 'code'`: Inline Cirru expression (one-liner).
- `-f file.cirru`: Multi-line code from a file (recommended for complex structures).
- `-j 'json'`: Raw JSON-serialized Cirru representation.

> Note: For multi-line text input, prefer using `-f` with a temporary file in `.calcit-snippets/`.

## Best Practices

1. **Check first**: Use `cr query find` or `cr tree view` to confirm the current state.
2. **From back to front**: When performing multiple `delete` or `insert` operations at the same level, start from the highest index to avoid shifting indices.
3. **Use target-replace**: It is usually safer than path-based replacement as it validates the current content.

---
title: "CLI Code Editing (edit & tree)"
summary: "如何使用 cr tree show/replace/search-replace/delete/batch-delete/insert/wrap/rewrite 查看和修改 AST 节点"
scope: "core"
kind: "reference"
category: "run"
aliases:
  - "edit tree"
  - "edit transaction"
  - "tree cursor"
  - "search-replace"
  - "add-import"
  - "tree replace"
  - "tree rewrite"
entry_for:
  - "cr tree search-replace"
  - "cr edit add-import"
  - "cr edit transaction"
  - "cr cursor"
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

### Persistent Tree Cursor

For a sequence of edits in one complex expression, `cr cursor` stores the active tree selection in `.calcit-cursor.cirru` next to the snapshot. The file is Cirru EDN local state and does not become part of the source snapshot:

```bash
cr calcit.cirru cursor set app.main/render! --path @3.2.1
cr calcit.cirru cursor show
cr calcit.cirru cursor parent
cr calcit.cirru cursor child             # first child
cr calcit.cirru cursor child 2
cr calcit.cirru cursor child --last      # last child
cr calcit.cirru cursor next --count 3
cr calcit.cirru cursor prev --count 2
cr calcit.cirru cursor forward --count 8
cr calcit.cirru cursor backward --count 5
cr calcit.cirru cursor back --count 4
cr calcit.cirru cursor push
cr calcit.cirru cursor pop
```

Definition-oriented `query`, `tree`, and `edit` commands accept `@cursor` as their target. Path-based commands also accept it as the path, so the current definition and selection do not need to be repeated:

```bash
cr calcit.cirru query context @cursor --format json
cr calcit.cirru query type-at @cursor --path @cursor --format json
cr calcit.cirru tree show @cursor --path @cursor
cr calcit.cirru tree replace @cursor --path @cursor \
  --code 'quote $ render-list items'
cr calcit.cirru edit split-def @cursor --path @cursor --name render-items
```

For commands with two definition targets, `@cursor` denotes the source (`edit mv-def @cursor app.ui/render-items`). An explicit target/path that disagrees with the active cursor is rejected. Transaction operation files remain self-contained and should use concrete targets and paths rather than depending on mutable cursor state.

For the most common mutations, `cursor apply` infers both the definition target and path from the active selection. The operation is still delegated to the existing `tree` implementation, so validation, cursor migration, and result previews stay identical:

```bash
cr calcit.cirru cursor apply swap-next
cr calcit.cirru cursor apply replace --code 'quote $ render-list items'
cr calcit.cirru cursor apply wrap --code 'quote $ when visible? self'
cr calcit.cirru cursor apply insert-after --file .calcit-snippets/branch.cirru
```

`cursor apply unwrap` splices every child of the selected list into its parent. It is not necessarily the inverse of a wrapper template containing extra syntax such as `quote $ do self`; use `raise` when the intent is to replace a parent with one selected child.

`edit cp`, `edit mv`, and `edit split-def` also accept `@cursor` in their path options. Definition overwrite, rename, move, split, and delete operations update the cursor target/path when the result is provable; ambiguous external or move changes still require a unique fingerprint match.

Once a cursor is set, successful tree mutations in that definition maintain it even when the command uses a concrete path. Inserting or deleting a sibling before the selected subtree shifts the saved coordinate; swaps follow the selected subtree; deleting the selected subtree moves the cursor to its parent. Cursor feedback is written to stderr and controlled by the top-level `--cursor-after none|summary|focus` option (`summary` is the default):

```text
[Cursor] app.main/render! @3.5 — @3.4 → @3.5 (node inserted before cursor)
```

```bash
cr --cursor-after focus calcit.cirru tree replace app.main/render! \
  --path @cursor --code 'quote $ render-list next-items'
```

`cursor show` reparses the snapshot and verifies the saved subtree fingerprint. If an external change invalidated the numeric path, a unique fingerprint match may relocate it; zero or multiple matches are rejected rather than guessed. The default `--view focus` uses Cirru's structural focus formatter on the surrounding definition and preserves its signature; `--view node` shows only the selection, while `--view full` shows the whole definition. Human display wraps only the presentation copy in `CURSOR`; `cursor show --format json` returns the real subtree as `tree` and the presentation tree as `preview_tree`, so the wrapper never changes source paths.

The cursor state has independent navigation history and an explicit stack:

```bash
cr calcit.cirru cursor back       # restore the previous cursor location; source is unchanged
cr calcit.cirru cursor push       # remember a location before a detour
cr calcit.cirru cursor pop        # restore that explicit location
```

`cursor child` defaults to child 0, while `cursor child --last` resolves the final child from the current tree. `next` and `prev` move only among siblings. `forward` and `backward` walk the whole definition in depth-first structural order, entering and leaving nested lists without requiring a separate `parent` or `child` command. All four accept `--count N`; `back` accepts the same option for history. A multi-step move is recorded as one history transition. Invalid zero counts and out-of-range moves leave the cursor unchanged. With top-level `--cursor-after focus`, selection and navigation commands immediately print the focused structural context.

`cursor back` rewinds only navigation state; it is not source undo. Parallel agents should use separate worktrees or Snapshots because the sidecar has one active cursor and does not coordinate concurrent source writes.

The first Paredit-style moves operate directly on the active selection:

```bash
cr calcit.cirru cursor slurp-next  # selected list absorbs its next sibling
cr calcit.cirru cursor slurp-prev  # selected list absorbs its previous sibling
cr calcit.cirru cursor barf-last   # selected list ejects its last child
cr calcit.cirru cursor barf-first  # selected list ejects its first child
cr calcit.cirru cursor duplicate --at after
```

The four slurp/barf commands keep the cursor attached to the selected list and cover both structural directions. `duplicate` selects the new copy without replacing the cursor clipboard. Composite changes stage the Snapshot and cursor sidecar before committing, and reject roots, leaves, empty lists, missing siblings, and unsupported positions before changing the Snapshot.

Search results expose a zero-based global cursor index as `[#N]` in human output and `cursor_index` in JSON. Use `--set-cursor N` to select a result in the same invocation:

```bash
cr calcit.cirru query search render-item \
  --filter app.main/render! --exact --set-cursor 0
cr calcit.cirru query search-expr 'map items' \
  --filter app.main/render! --set-cursor 1 --format json
cr calcit.cirru query search state --start-path @cursor --set-cursor 0
cr calcit.cirru query search-expr 'div $ {}' --start-path @cursor
```

`--filter @cursor` searches the active definition. `--start-path @cursor` further restricts either leaf or expression search to the selected subtree and automatically infers the definition filter; a conflicting explicit filter is rejected. The cursor confirmation is written to stderr, so JSON stdout remains one parseable object. Search results from configured dependencies remain readable, but cannot become the editable project cursor unless that definition also belongs to the current snapshot.

Its clipboard stores a quoted Cirru tree directly in `.calcit-cursor.cirru`, so code never round-trips through JSON or escaped text:

```bash
cr calcit.cirru cursor copy
cr calcit.cirru cursor cut
cr calcit.cirru cursor clipboard --format json
cr calcit.cirru cursor paste --at before
cr calcit.cirru cursor clear-clipboard
```

`cut` moves the selection to its parent; `paste` selects the inserted expression and keeps the clipboard available for repeated paste. Both commands stage Snapshot and sidecar output before committing: cut persists the recoverable clipboard first, while paste reports explicitly if the Snapshot succeeded but cursor state did not, so callers must not retry that partial-success case blindly. Cursor schema v3 keeps full Cirru only in the clipboard, not in every history/stack entry. Add `.calcit-cursor.cirru` to the project `.gitignore`; `cursor set` prints a warning when it cannot find a matching rule.

Use `cr calcit.cirru cursor clear` to remove the local selection and clipboard together. Transaction child operations deliberately do not mutate the real cursor file; after a committed transaction, the parent command revalidates or uniquely relocates the cursor against the final snapshot and warns if manual recovery is required.

### Atomic Transactions

`cr edit transaction` applies existing `edit`, `tree`, and `config` mutations to a staged snapshot. The original snapshot is replaced only after every operation succeeds and the staged result can be loaded and serialized again. A failed operation, stale revision, or `--dry-run` leaves the original file unchanged.

The primary input format is a Cirru EDN list of CLI argument lists. Each inner list is exactly the argument sequence that would follow the snapshot path in an ordinary command. A `quote` value can be embedded directly after `--code`, so multiline Calcit code remains structured rather than escaped inside a string:

```cirru.no-check
[]
  [] |edit |doc |app.main/main! "|Updated by transaction"
  []
    , |tree
    , |replace
    , |app.main/main!
    , |--path
    , |@3.2
    , |--code
    quote $ println |done
```

```bash
# Preview and obtain the current snapshot revision.
cr calcit.cirru edit transaction --file changes.cirru --dry-run --format json

# Commit only if the snapshot still has that revision.
cr calcit.cirru edit transaction --file changes.cirru \
  --expect-revision md5:... --format json
```

JSON argument lists remain accepted as a compatibility format for callers that already construct JSON, but JSON is not the recommended authoring format when operations contain Calcit code:

```json
[
  ["edit", "doc", "app.main/main!", "Updated by transaction"],
  ["tree", "replace", "app.main/main!", "--path", "@3.2", "--code", "quote $ println |done"]
]
```

Nested transactions and non-mutating command groups are rejected. JSON output contains the old/new snapshot revision and captures each existing subcommand's stdout/stderr without mixing it into the transaction stdout.

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
# Replace numeric leaf '1' with '10' inside the definition
cr tree search-replace app.main/main! --pattern '1' --code 'quote 10'
```

### Path-based Operations

You can use numeric paths to locate deep nodes:

```bash
# Replace the node at path @1.2.0
cr tree replace app.main/main! --path '@1.2.0' --code 'quote $ + 1 2'

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

- symbol leaf: `--code 'quote leaf'`
- string leaf: `--code 'quote |text'`
- expression: `--code 'quote $ expr ...'` or `--code 'quote (expr ...)'`
- stdin / `--file` likewise use `quote ...`
- only JSON array input can be passed without `quote`

`cr edit schema` follows the same rule and accepts exactly one quoted type node. `cr edit examples` is the batch form: each top-level item must independently be `quote |leaf` or `quote $ expr ...`. A quoted `[]` wrapper is not a batch marker because it would describe one AST node, not a collection of edit operations.

> Note: For multi-line text input, prefer `--file` or stdin heredoc. They avoid shell escaping, but Cirru content still needs the `quote` prefix.

## Best Practices

1. **Check first**: Use `cr query find` or `cr tree show` to confirm the current state.
2. **From back to front**: When performing multiple `delete` or `insert` operations at the same level, start from the highest index to avoid shifting indices.
3. **Use search-replace**: It is usually safer than path-based replacement as it validates the current content.

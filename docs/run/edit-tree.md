---
title: "CLI Code Editing (edit & tree)"
summary: "如何使用 calcit tree show/replace/search-replace/delete/batch-delete/insert/wrap/rewrite 查看和修改 AST 节点"
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
  - "calcit tree search-replace"
  - "calcit edit add-import"
  - "calcit edit transaction"
  - "calcit cursor"
  - "calcit query search"
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

## Core Editing (calcit edit)

The `edit` command handles high-level operations on namespaces and definitions.

```bash
# Refresh snapshot formatting without semantic changes
calcit edit format
```

This command also rewrites older namespace records into canonical shapes. Namespace keys under `:files` and definition keys under `:defs` are written as Symbols; readers continue accepting legacy String keys. A snapshot containing both forms of the same normalized identifier is rejected instead of silently overwriting one entry. A retired `compact.cirru` must first be copied or renamed to `calcit.cirru`; then `edit format` alone can migrate early direct-quote code and top-level `:configs` through an isolated one-way loader. It reports migrated node counts and rejects ambiguous legacy configs. Runtime loading and other edits remain strict. A direct-quote `defmacro` is the narrow exception to ordinary Dynamic initialization: its parameter markers become conservative `Syntax` required/optional/rest slots, its expansion is `Expr<Dynamic>`, and its capability set is empty, so the canonical result is immediately readable without granting effects. Existing structured Dynamic macro schemas remain rejected. For accepted snapshots, stderr identifies `W_LEGACY_ANY` or `W_DYNAMIC_TYPE_DEBT` when follow-up work is recommended. It does not invent concrete semantic types; follow dynamic or unbound-slot warnings with `calcit analyze weak-types --only schema-dynamic,unresolved-type-slot,code-dynamic --intent unresolved`.

### Schema 缺省与显式 Dynamic

未声明 `:schema` 和显式 `Dynamic` 不是同一意图：前者表示缺少声明，后者表示用户选择开放类型边界。`edit def` 创建未标注定义，以及后续结构化编辑、`edit format`，均保留缺省状态，不自动写入 `Dynamic`。

`calcit edit schema app.main/helper --clear` 删除已有 schema 字段，而不是声明 `Dynamic`。如需显式开放声明，使用 `calcit edit schema app.main/helper --input-format cirru --code "quote $ :: 'Dynamic"`。两种状态拥有不同的 definition revision，不能混用旧的编辑前置条件或分析证据。

清除声明不等于通过类型检查，也不启用新的推断能力；当前严格模式仍要求无法证明的函数边界提供结构化 `Fn` 契约。编辑后继续运行 `calcit --check-only`。函数体推断由 [#1307](https://github.com/calcit-lang/calcit/issues/1307) 跟进，不应通过自动补写 `Dynamic` 绕过。

### Persistent Tree Cursor

For a sequence of edits in one complex expression, `calcit cursor` stores the active tree selection in `.calcit/cursor.cirru` next to the snapshot. `.calcit/` is the shared project-local state directory for the cursor, recent error stack, snippets, and other bounded local artifacts; it does not become part of the source snapshot:

```bash
calcit calcit.cirru cursor set app.main/render! --path @3.2.1
calcit calcit.cirru cursor show
calcit calcit.cirru cursor parent
calcit calcit.cirru cursor child             # first child
calcit calcit.cirru cursor child 2
calcit calcit.cirru cursor child --last      # last child
calcit calcit.cirru cursor next --count 3
calcit calcit.cirru cursor prev --count 2
calcit calcit.cirru cursor forward --count 8
calcit calcit.cirru cursor backward --count 5
calcit calcit.cirru cursor back --count 4
calcit calcit.cirru cursor push
calcit calcit.cirru cursor pop
calcit calcit.cirru cursor anchor
calcit calcit.cirru cursor region
calcit calcit.cirru cursor clear-anchor
calcit calcit.cirru cursor mark render-start
calcit calcit.cirru cursor goto render-start
calcit calcit.cirru cursor marks
calcit calcit.cirru cursor rm-mark render-start
```

Definition-oriented `query`, `tree`, and `edit` commands accept `@cursor` as their target. Path-based commands also accept it as the path, so the current definition and selection do not need to be repeated:

```bash
calcit calcit.cirru query context @cursor --format edn
calcit calcit.cirru query type-at @cursor --path @cursor --format edn
calcit calcit.cirru tree show @cursor --path @cursor
calcit calcit.cirru tree replace @cursor --path @cursor \
  --input-format cirru --code 'quote $ render-list items'
calcit calcit.cirru edit split-def @cursor --path @cursor --name render-items
```

For commands with two definition targets, `@cursor` denotes the source (`edit mv-def @cursor app.ui/render-items`). An explicit target/path that disagrees with the active cursor is rejected. Transaction operation files remain self-contained and should use concrete targets and paths rather than depending on mutable cursor state.

For the most common mutations, `cursor apply` infers both the definition target and path from the active selection. The operation is still delegated to the existing `tree` implementation, so validation, cursor migration, and result previews stay identical:

```bash
calcit calcit.cirru cursor apply swap-next
calcit calcit.cirru cursor apply replace --input-format cirru --code 'quote $ render-list items'
calcit calcit.cirru cursor apply wrap --input-format cirru --code 'quote $ when visible? self'
calcit calcit.cirru cursor apply insert-after --input-format cirru --file .calcit/snippets/branch.cirru
```

`cursor apply unwrap` splices every child of the selected list into its parent. It is not necessarily the inverse of a wrapper template containing extra syntax such as `quote $ do self`; use `raise` when the intent is to replace a parent with one selected child.

`edit cp`, `edit mv`, and `edit split-def` also accept `@cursor` in their path options. Definition overwrite, rename, move, split, and delete operations update the cursor target/path when the result is provable; ambiguous external or move changes still require a unique fingerprint match.

Once a cursor is set, successful tree mutations in that definition maintain it even when the command uses a concrete path. Inserting or deleting a sibling before the selected subtree shifts the saved coordinate; swaps follow the selected subtree; deleting the selected subtree moves the cursor to its parent. Cursor feedback is written to stderr and controlled by the top-level `--cursor-after none|summary|focus` option (`summary` is the default):

```text
[Cursor] app.main/render! @3.5 — @3.4 → @3.5 (node inserted before cursor)
```

```bash
calcit --cursor-after focus calcit.cirru tree replace app.main/render! \
  --path @cursor --input-format cirru --code 'quote $ render-list next-items'
```

`cursor show` reparses the snapshot and verifies the saved subtree fingerprint. If an external change invalidated the numeric path, a unique fingerprint match may relocate it; zero or multiple matches are rejected rather than guessed. The default `--view focus` uses Cirru's structural focus formatter on the surrounding definition and preserves its signature; `--view node` shows only the selection, while `--view full` shows the whole definition. Human display wraps only the presentation copy in `CURSOR`; `cursor show --format json` returns the real subtree as `tree` and the presentation tree as `preview_tree`, so the wrapper never changes source paths.

The cursor state has independent navigation history and an explicit stack:

```bash
calcit calcit.cirru cursor back       # restore the previous cursor location; source is unchanged
calcit calcit.cirru cursor push       # remember a location before a detour
calcit calcit.cirru cursor pop        # restore that explicit location
```

`cursor child` defaults to child 0, while `cursor child --last` resolves the final child from the current tree. `next` and `prev` move only among siblings. `forward` and `backward` walk the whole definition in depth-first structural order, entering and leaving nested lists without requiring a separate `parent` or `child` command. All four accept `--count N`; `back` accepts the same option for history. A multi-step move is recorded as one history transition. Invalid zero counts and out-of-range moves leave the cursor unchanged. With top-level `--cursor-after focus`, selection and navigation commands immediately print the focused structural context.

`cursor back` rewinds only navigation state; it is not source undo. Parallel agents should use separate worktrees or Snapshots because the sidecar has one active cursor and does not coordinate concurrent source writes.

For a temporary contiguous sibling range, set one anchor and move the active cursor to the other endpoint. `cursor region` reparses both saved positions, requires the same definition and parent, normalizes either direction, and displays the selected trees without changing their real paths. The first version deliberately does not make single-expression `copy` or `cut` operate on the region implicitly:

```bash
calcit calcit.cirru cursor anchor
calcit calcit.cirru cursor next --count 3
calcit calcit.cirru cursor region --format json
calcit calcit.cirru cursor clear-anchor
```

Named marks are bounded location bookmarks, not additional active cursors. At most 16 are stored, and each contains only target/path/revision/fingerprint. Use them for repeated cross-definition movement; use `push/pop` for a short detour.

The first Paredit-style moves operate directly on the active selection:

```bash
calcit calcit.cirru cursor slurp-next  # selected list absorbs its next sibling
calcit calcit.cirru cursor slurp-prev  # selected list absorbs its previous sibling
calcit calcit.cirru cursor barf-last   # selected list ejects its last child
calcit calcit.cirru cursor barf-first  # selected list ejects its first child
calcit calcit.cirru cursor duplicate --at after
```

The four slurp/barf commands keep the cursor attached to the selected list and cover both structural directions. `duplicate` selects the new copy without replacing the cursor clipboard. Composite changes stage the Snapshot and cursor sidecar before committing, and reject roots, leaves, empty lists, missing siblings, and unsupported positions before changing the Snapshot.

Search results expose a zero-based global cursor index as `[#N]` in human output and `cursor_index` in JSON. Use `--set-cursor N` to select a result in the same invocation:

```bash
calcit calcit.cirru query search render-item \
  --filter app.main/render! --exact --set-cursor 0
calcit calcit.cirru query search-expr 'map items' \
  --filter app.main/render! --set-cursor 1 --format json
calcit calcit.cirru query search state --start-path @cursor --set-cursor 0
calcit calcit.cirru query search-expr 'div $ {}' --start-path @cursor
calcit calcit.cirru query next
calcit calcit.cirru query prev
```

`--filter @cursor` searches the active definition. `--start-path @cursor` further restricts either leaf or expression search to the selected subtree and automatically infers the definition filter; a conflicting explicit filter is rejected. For editable searches, add `--source project`; the backward-compatible default is `all`. The cursor confirmation is written to stderr, so JSON stdout remains one parseable object. Search results from configured dependencies remain readable, but cannot become the editable project cursor unless that definition also belongs to the current snapshot. A successful `--set-cursor` saves the source scope with the other query arguments, selected index, and snapshot revision. `query next/prev` reruns that same scoped query and moves to the adjacent result; it never persists the full result list. If the snapshot changed, it rejects the stale index and asks for an explicit search selection instead of risking a reordered match.

Its clipboard stores a quoted Cirru tree directly in `.calcit/cursor.cirru`, so code never round-trips through JSON or escaped text:

```bash
calcit calcit.cirru cursor copy
calcit calcit.cirru cursor cut
calcit calcit.cirru cursor clipboard --format json
calcit calcit.cirru cursor paste --at before
calcit calcit.cirru cursor clear-clipboard
```

`cut` moves the selection to its parent; `paste` selects the inserted expression and keeps the clipboard available for repeated paste. Both commands stage Snapshot and sidecar output before committing: cut persists the recoverable clipboard first, while paste reports explicitly if the Snapshot succeeded but cursor state did not, so callers must not retry that partial-success case blindly. Cursor schema v4 keeps full Cirru only in the clipboard, not in history, stack, anchor, marks, or last-query. History, stack, and marks are bounded, and the whole file has a 64 KiB hard limit. Add `.calcit/` to the project `.gitignore`; `cursor set` prints a warning when it cannot find a matching rule. Existing `.calcit-cursor.cirru` is moved to the new path on first read, without dual writes.

Use `calcit calcit.cirru cursor clear` to remove the local selection and clipboard together. Transaction child operations deliberately do not mutate the real cursor file; after a committed transaction, the parent command revalidates or uniquely relocates the cursor against the final snapshot and warns if manual recovery is required.

### Architecture Scaffold

`calcit edit scaffold` turns one feature-level architecture plan into a validated
set of definition work items. The recommended source is Cirru EDN under
`docs/architectures/`; the canonical graph uses Symbol FQNs in `:definitions`
and homogeneous typed anonymous-enum edges such as
`:: :call 'app.order/validate 'app.order/order-total`.

Preview first:

```bash
calcit calcit.cirru edit scaffold \
  --file docs/architectures/order-submission.cirru \
  --dry-run --format edn
```

Apply with a revision precondition:

```bash
calcit calcit.cirru edit scaffold \
  --file docs/architectures/order-submission.cirru \
  --expect-revision md5:... --format edn
```

Scaffold reconciliation keeps existing definitions in the result. Compatible
definitions become `reuse-complete` or `reuse-pending`; documentation and
Dynamic-schema differences are warnings; concrete kind/schema conflicts reject
the whole apply. Only missing `:ensure` definitions are created. Function stubs
contain `todo!` and are marked `:scaffold`; data definitions must provide an
explicit quoted `:code`.

The apply stages the complete Snapshot, rechecks its revision, and commits with
one atomic rename. A successful apply then revalidates the current cursor. The
work item is the unit assigned to an Agent; the first workflow keeps Snapshot
writes serial at the parent/coordinator even when implementation work is
parallel.

### Atomic Transactions

`calcit edit transaction` applies existing `edit`, `tree`, and `config` mutations to a staged snapshot. The original snapshot is replaced only after every operation succeeds and the staged result can be loaded and serialized again. A failed operation, stale revision, or `--dry-run` leaves the original file unchanged.

仅修改 Snapshot 的 `edit ffi` 元数据可与 `edit schema` 和 tree 修改一起进入事务。这不会将外部 JavaScript 文件纳入事务或写入文件；`edit ffi --file` 只读取输入，然后更新 Snapshot。

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
    , |--input-format
    , |cirru
    , |--code
    quote $ println |done
```

```bash
# 预览并读取当前 Snapshot revision。
calcit calcit.cirru edit transaction --file changes.cirru --dry-run --format edn

# 仅在 Snapshot 仍是该 revision 时提交。
calcit calcit.cirru edit transaction --file changes.cirru \
  --expect-revision md5:... --format edn
```

JSON argument lists remain accepted as a compatibility format for callers that already construct JSON, but JSON is not the recommended authoring format when operations contain Calcit code:

```json
[
  ["edit", "doc", "app.main/main!", "Updated by transaction"],
  ["tree", "replace", "app.main/main!", "--path", "@3.2", "--input-format", "cirru", "--code", "quote $ println |done"]
]
```

嵌套 transaction 与非 mutation 命令组会被拒绝。默认 human 输出使用 Markdown heading、metadata list 与 fenced operation output；`--format edn` 返回相同语义的稳定 Cirru EDN envelope，包含新旧 Snapshot revision，并分别捕获每条子命令的 stdout/stderr。只有对接 JSON-only consumer 时才使用 `--format json`。

### Agent 的 read → preview → apply → verify 流程

以下流程把源码边界与控制信息分开，也避免 Agent 在两次读取之间覆盖并发修改：

```bash
# read：先以 Markdown 查看目标代码，代码只出现在 fenced Cirru block 中。
calcit calcit.cirru tree show app.main/main! --path @3

# preview：dry-run 不写文件，从 EDN envelope 读取 :original-revision、:changed 与 :operations。
calcit calcit.cirru edit transaction --file changes.cirru --dry-run --format edn

# apply：原样使用 preview 的 operation 文件，并绑定返回的 revision。
calcit calcit.cirru edit transaction --file changes.cirru \
  --expect-revision 'md5:<original-revision>' --format edn

# verify：重新读取目标，再运行项目的严格检查与相关测试。
calcit calcit.cirru tree show app.main/main! --path @3
calcit calcit.cirru --check-only
calcit calcit.cirru test app.main/main! --require-match
```

`tree`、`cursor apply` 与 `fix` 的 human preview 都使用同一组 Markdown 边界：operation、path、revision、changed 等控制信息位于 fence 外，Before/After 源码位于 `cirru` fence 内。guard 失败也以 Expected/Actual 两个 fence 输出到 stderr。不要把 heading、列表项或截断提示复制回 Snapshot。

### Managing Namespaces

```bash
# Move a definition to another namespace
calcit edit mv-def app.main/old-name app.util/old-name

# Rename a definition within the same namespace
calcit edit rename app.main/old-name new-name

# Add a new namespace
calcit edit add-ns app.util

# Remove a namespace
calcit edit rm-ns app.util
```

### Managing Imports

```bash
# Add an import to a namespace
calcit edit add-import app.main --input-format cirru --code 'quote (respo.core :refer $ deftime)'

# Bulk reset all imports for a namespace
calcit edit imports app.main --input-format cirru --file imports.cirru
```

`imports.cirru` quotes one Cirru imports vector node. The command removes the outer `quote`, checks the `[]` marker, and uses each child expression as one import rule:

```cirru
quote $ []
  respo.core :refer $ div span
  respo-ui.core :as ui
```

新调用应显式传 `--input-format cirru`。需要 JSON 时，`--input-format json-ast` 接收同一个完整节点，例如
`["[]",["respo.core",":refer",["div","span"]]]`；JSON 只用于必须对接 JSON 的 consumer。
默认的 `auto` 继续接受旧的规则数组 `[["respo.core",...]]`，但仅作为兼容读取。不要包含外层
`:require`；命令会自行重建。

### Managing Schemas and Examples

```bash
# Schema accepts exactly one quoted Cirru type node.
calcit edit schema 'app.main/*enabled?' --input-format cirru --code "quote $ :: 'Ref 'Bool"

# Concrete defstruct/defenum value schemas use a fully qualified nominal type.
calcit edit schema app.schema/store --input-format cirru --code "quote 'app.schema/Store"

# Each top-level quote becomes one example; leaves remain representable.
calcit edit examples app.main/add << 'END'
quote $ add 1 2
quote $ add 3 4
quote |literal
END

# Execute only the examples attached to one definition.
calcit analyze check-examples --ns app.main --def add
```

## Fine-grained AST Operations (calcit tree)

The `tree` command allows precise manipulation of nodes within a definition's S-expression tree.

### Viewing the Tree

```bash
# View the AST of a definition with indices
calcit tree show app.main/main!
```

### Target-based Replacement

`search-replace` is the safest way to modify a specific node by its content:

```bash
# Replace numeric leaf '1' with '10' inside the definition
calcit tree search-replace app.main/main! --pattern '1' --input-format cirru --code 'quote 10'
```

### Path-based Operations

You can use numeric paths to locate deep nodes:

```bash
# Replace the node at path @1.2.0
calcit tree replace app.main/main! --path '@1.2.0' \
  --expect 'quote old-value' --input-format cirru --code 'quote $ + 1 2'

# Insert before a node
calcit tree insert-before app.main/main! --path '@1.0' \
  --expect 'quote (render-page)' --input-format cirru --code 'quote (println |started)'

# Delete a node
calcit tree delete app.main/main! --path '@1.0' --expect 'quote (render-page)'
```

`--expect` is optional, but recommended whenever a numeric path is copied from
an earlier query. The mutation is rejected before writing if the node (or insert
anchor) no longer has the expected shape. This catches both stale indices and a
path copied from an adjacent sibling. Use quoted Cirru or a JSON AST, following
the same input rules as `--code`.

### Copying and Moving Nodes

Current CLI exposes node copy/move under `calcit edit cp` and `calcit edit mv`:

```bash
# Copy a node within a definition
calcit edit cp app.main/target-def --from '@1.0' --path '@2.0' --at append-child

# Move a node within a definition
calcit edit mv app.main/target-def --from '@1.0' --path '@2.0' --at after
```

## 输入格式

新增自动化必须显式指定 syntax-node 的传输格式；这只是现有 mutation 命令的共同选项，不是新的工具入口：

- `--input-format cirru`：`--code`、`--file` 或 stdin 中必须提供带 `quote` 的 Cirru EDN。
- `--input-format json-ast`：JSON 字符串表示 leaf，JSON 数组表示 list。
- `--input-format auto`：兼容旧脚本的默认值，会按内容识别 JSON 数组或 quoted Cirru；新调用不要依赖它。

```bash
# symbol leaf
calcit tree replace app.main/main! --path @2 \
  --input-format cirru --code 'quote leaf'

# expression
calcit tree replace app.main/main! --path @2 \
  --input-format json-ast --code '["expr","1"]'
```

`json-ast` 能无歧义地区分 JSON 字符串 `"[]"`、JSON 空数组 `[]` 与表达式数组 `["[]"]`。Cirru 输入仍保留
`quote` 的语言语义：`quote $ []` 解码为包含 `[]` operator 的表达式，不等于空 syntax list。

显式格式会在 mutation 前输出格式、节点类型、canonical JSON AST 与结构化摘要。`calcit edit schema`、
`edit def`、`edit add-example`、`edit add-test`、`edit add-ns`、`edit add-import`，以及接受新节点的
`tree` / `cursor apply` 操作共用这个契约。`edit examples` 等批量数据格式继续保留各自的集合协议，不把集合与单节点
混为一谈。

多行内容优先使用 `--file` 或 stdin，避免 shell 转义；选择 `cirru` 时内容依然必须包含 `quote`。`--expect` 始终使用
quoted Cirru guard，独立于新节点的传输格式。

## Best Practices

1. **Check first**: Use `calcit query find` or `calcit tree show` to confirm the current state.
2. **From back to front**: When performing multiple `delete` or `insert` operations at the same level, start from the highest index to avoid shifting indices.
3. **Use search-replace**: It is usually safer than path-based replacement as it validates the current content.

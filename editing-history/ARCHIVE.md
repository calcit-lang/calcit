# Consolidated history through June 2026

Detailed task notes before July 2026 were consolidated on 2026-08-01. Their
full text remains recoverable from Git history; this file preserves the useful
navigation layer.

| Period | Durable themes |
| --- | --- |
| 2025–Feb 2026 | WASM/codegen groundwork, structural equality, diagnostics, core API documentation, and migration toward `defstruct`. |
| Mar 2026 | Schema and type-annotation redesign: `hint-fn` migration, schema normalization/validation, generic type references, arity checks, and type-fail coverage. |
| Apr 2026 | Records/enums, type narrowing and generic dispatch; the WASM runtime gained collections, strings, methods, host imports, and multi-module tests. |
| May–Jun 2026 | Weak-type analysis, data-definition bounds, markdown checking, compiled-runtime backfill, scoped type slots, effects graph, and Agent/CLI documentation. |

## Pruning (Oct 2026)

Release notes for 0.12.x–0.15.x, review follow-ups, mechanical renames and
records about retired features (Calx target, quality budgets, legacy Optional
schemas, early Option/nil migrations) were removed from this directory on
2026-10-21. They remain recoverable from Git history; version-by-version
changes are in the release tags and their PRs.

For current behavior, prefer `docs/`, tests, RFCs and the retained records over
this summary.

## Consolidated development topics (Jul–Aug 2026)

### Cursor and transactional structural editing (Jul 28–29)

The CLI gained a project-local persistent cursor for safe, multi-command tree
editing. It supports navigation/history, selection search, clipboard actions,
focus previews, depth-first movement, symmetric slurp/barf operations, and
duplication. `@cursor` may resolve a definition target and tree path, but
two-target edits retain explicit destinations and transaction files remain
self-contained. Snapshot and cursor-sidecar writes are staged and report
partial success explicitly; a cursor is navigation state, never a concurrency
primitive or source identity. The authoritative interface and examples are in
`docs/run/edit-tree.md`, `docs/CalcitAgent.md`, and the cursor RFC.

### Project module cache hardening (Aug 17)

Runtime package-style module resolution is restricted to a project's
`.calcit/modules/` view, while immutable revisions live in the global
`module-caches/` store. Cache metadata preserves the highest observed SemVer
reference; `caps clean` retains the newest revision and every revision still
linked by a registered project. Cache and project-view updates are serialized
with OS-managed locks and installation is transactional. Current operational
details live in `docs/run/load-deps.md` and `docs/run/upgrade.md`.

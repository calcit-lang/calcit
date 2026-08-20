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

## Release history (Jul–Aug 2026)

Release and release-policy notes are consolidated here. Each release
synchronizes `Cargo.toml` / `package.json` and refreshes the workspace lockfile
with `cargo update --workspace`; the full note text remains recoverable from
Git history.

| Version | Durable context |
| --- | --- |
| 0.12.52 | Routine version bump with lockfile refresh. |
| 0.12.53 | Routine version bump. |
| 0.12.54 | Entry-level `:type-slots` configuration, CLI management, compile-time `with-type-slot` erasure, snapshot/config preservation. |
| 0.12.55 | Semantic snapshot entry descriptions and canonical symbol storage for entry functions. |
| 0.12.56 | Routine version bump after snapshot/entry work. |
| 0.12.57 | CLI mutation, atomic staging, test deduplication (PR #284) plus Windows staged-file synchronization fixes. |
| 0.13.0 | Concentrated breaking-change baseline for the Option/Result/Unit/JsNullish public contracts. |
| 0.13.1 | Fixed recursive expansion of self-referential nominal struct type annotations. |
| 0.13.2 | Struct/enum data model cleanup (PR #301), direct required Struct field access, symbol-based nominal formatting. |
| 0.13.3 | Struct/Enum terminology migration, deprecated API analysis, cross-backend canonical EDN formatting. |
| 0.13.4 | Typed host FFI (external-object traits) merged and released. |
| 0.12.58 | Receiver-first method inference and nominal trait identity fixes. |
| 0.12.59 | Top-level nominal value schema fix. |
| 0.13.5 | Typed `js-get` / `js-set` field access plus named-type preservation. |
| 0.13.6 | Opt-in raw-JS-FFI warning and readable low-cost JS emission. |
| 0.13.7 | Definition-attached core tests and safer, less noisy `cr` edits. |
| 0.13.8 | Typed Struct field access and robust branch-ref resolution. |
| 0.13.9 | Routine aligned Cargo/npm release. |
| 0.13.10 | Agent migration convergence and quoted-Cirru edit reliability. |
| 0.13.11 | Trailing `Option` parameters, typed Option/Struct handling, and terminating JS string replacement. |
| 0.13.12 | Type guidance/Dynamic audits and typed Struct constructors with omitted Option fields. |
| 0.13.13 | Project module-store and dependency-boundary work. |
| 0.13.15 | `decode-map-as` runtime decoder and core metadata. |
| 0.13.17 | Nested Struct context, migration diagnostics, and agent guidance. |
| 0.13.18 | Correct source locations for macro-generated field warnings. |
| 0.13.19 | Project-scoped module resolution, immutable cache visibility, and safe cleanup. |
| 0.13.20 | Typed JavaScript FFI capability boundary and browser/Node validation. |
| 0.13.21 | Typed JavaScript FFI follow-ups and `deps.cirru` version migration diagnostics. |

Also consolidated here: the 2026-08-01 ARCHIVE consolidation note, and the
release main-branch policy note — `main` needs no pull-request protection for
verified changes, the stable version commit runs a fresh main-branch CI run
before tagging, and npm verification uses the published package name
`@calcit/procs`.

For current behavior, prefer `docs/`, tests, and the retained current-window
notes over this summary.

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

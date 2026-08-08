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

Non-functional version-bump notes were consolidated here on 2026-08-08. Each
release synchronizes `Cargo.toml` / `package.json` and refreshes the workspace
lockfile with `cargo update --workspace`; the full note text remains recoverable
from Git history.

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

Also consolidated here: the 2026-08-01 ARCHIVE consolidation note, and the
release main-branch policy note — `main` needs no pull-request protection for
verified changes, the stable version commit runs a fresh main-branch CI run
before tagging, and npm verification uses the published package name
`@calcit/procs`.

For current behavior, prefer `docs/`, tests, and the retained current-window
notes over this summary.

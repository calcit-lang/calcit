---
title: "CLI Options"
scope: "core"
kind: "reference"
category: "run"
aliases:
  - "watch mode"
  - "watch"
  - "once mode"
  - "check-only"
  - "reload-fn"
  - "reload fn"
  - "watch-dir"
  - "macro metrics"
  - "macro expansion metrics"
  - "ffi metrics"
  - "async ffi metrics"
  - "ffi export"
entry_for:
  - "calcit -w"
  - "calcit js -w"
  - "calcit --help"
  - "calcit --reload-fn"
  - "calcit --macro-metrics"
  - "calcit --ffi-metrics"
  - "calcit ffi export"
---

# CLI Options

```bash
calcit --help
```

Quick note: `calcit edit format` rewrites the target snapshot using canonical serialization without guessing semantic changes. The retired `compact.cirru` filename must first be copied or renamed to `calcit.cirru`. Formatting alone has an isolated one-way loader for early direct-quote definitions/namespaces and top-level `:configs`; runtime loading and other commands remain strict. It reports migrated node counts, rejects ambiguous or unknown legacy config fields, and initializes migrated definitions with explicit `Dynamic` schemas. Current formatting also normalizes older namespace records and rewrites legacy schema type tags such as `:string` and `:ref` to quoted symbols such as `'String` and `'Ref` only in known type positions. Ordinary tag data stays unchanged. It emits recoverable stderr advisories for legacy `:any` and unresolved dynamic type debt. Use `calcit analyze weak-types` for exact paths and recommendations; format warnings do not turn the command into a type-quality gate.

For feature-level planning, use `calcit edit scaffold`. Its primary input is a
Cirru EDN architecture plan, preferably stored under
`docs/architectures/<feature>.cirru`:

```bash
calcit calcit.cirru edit scaffold --file docs/architectures/order.cirru \
  --dry-run --format edn
calcit calcit.cirru edit scaffold --file docs/architectures/order.cirru \
  --expect-revision md5:... --format edn
```

`--dry-run` previews reconciliation and work items without writing. Apply mode
atomically creates missing definitions only; existing definitions are reported
with their planned/existing metadata and are never overwritten. EDN is the
canonical machine format; JSON is a compatibility projection.

## Detailed Option Descriptions

### Typed FFI inventory (`ffi export`)

Export local raw bindings declared by non-empty `:ffi` lowering metadata:

```bash
calcit calcit.cirru ffi export
calcit calcit.cirru ffi export --json --ns app.ffi
```

The JSON mode keeps stdout to one machine-readable document and reports
unsupported schemas as deterministic diagnostics rather than Dynamic
fallbacks. See [FFI Interface IR](../installation/ffi-interface-ir.md) for the
versioned schema and boundary rules.

### Input File

```bash
# Run default calcit.cirru
calcit

# Run specific file
calcit calcit.cirru
```

### Run Mode (default once)

By default, `calcit` runs once and exits. Use `--watch` (`-w`) to enable watch mode:

```bash
calcit --watch
calcit -w calcit.cirru
```

### Error Stack Trace (--disable-stack)

Disables detailed stack traces in error messages, useful for cleaner output:

```bash
calcit --disable-stack
```

### JS Codegen Options

**--skip-arity-check**: When generating JavaScript, skip arity checking (use cautiously):

```bash
calcit js --skip-arity-check
```

**--emit-path**: Specify output directory for generated JavaScript:

```bash
calcit js --emit-path dist/
```

### Dynamic Method Warnings (--warn-dyn-method)

Warn when dynamic method dispatch cannot be specialized at preprocess time, and surface related trait-attachment diagnostics:

```bash
calcit --warn-dyn-method
```

### Strict type preflight (`--strict-types`)

Use `--strict-types` for new or fully migrated modules that must carry no local
type debt:

```bash
calcit --check-only --strict-types
calcit --strict-types js
```

The flag enables the location-aware untyped JS FFI diagnostics from
`--warn-dyn-method`, then runs the zero-baseline static quality gate before
execution or code generation. It rejects unresolved or schema `Dynamic`, code
`nil`, declared legacy optional values, deprecated calls, and explicit
`unsafe-coerce` boundaries. Deep/open payloads may still use `Dynamic`, but a
project that intentionally retains such boundaries should document and freeze
them with `calcit analyze quality --baseline <file>` instead of claiming the
zero-debt strict policy.

Strict preprocessing also rejects two constructs that manufacture `nil`
implicitly:

- `E_LEGACY_OPTIONAL_PARAM`: a `?` parameter would bind an omitted argument to
  `nil`. Remove the marker, declare trailing parameters as `Option<T>`, and let
  omission insert `%none` (or pass `%some value` / `%none` explicitly).
- `E_PARTIAL_STRUCT_NIL_FILL`: `%{}?` / `&%{}?` would fill omitted Struct fields
  with `nil`. Use `%{}` and provide every field explicitly; an `Option<T>` field
  receives `%none` rather than `nil`.
- `E_NIL_FOR_UNIT`: a function declared to return `Unit` actually returns the
  distinct `Nil` value. Replace returned `nil` / `;nil` with `&unit`, or end the
  body with an effect that already returns Unit. Intermediate nil expressions
  are not classified as the function return.
- `E_NIL_CALLBACK_SENTINEL`: an inline `map-kv` callback has a structurally
  visible return path that uses `nil` to drop an entry. Use `filter-map-kv` and
  return `MapEntryDecision :keep key value` or `MapEntryDecision :drop` on every
  path. A nil nested inside the returned pair remains map data.

These compatibility paths remain available outside `--strict-types` during
ecosystem migration. Partial Struct construction is not auto-fixed because the
compiler cannot infer whether an omitted business field should become
`Option<T>`, gain a default, or be supplied by the caller.

Strict preprocessing also reports `E_BARE_CONTAINER_SCHEMA` when a public
function or macro contract uses bare `List`, `Map`, `Set`, or `Ref`. The error
includes the exact schema path whose missing type argument became Dynamic. Add
concrete arguments, use a declared `:generics` variable when positions are
related, or spell `Dynamic` explicitly only for a reviewed open boundary. For
example, migrate `List` to `(:: List T)` with `:generics ([] T)`; an intentional
open boundary is written `(:: List Dynamic)` so analysis can distinguish it
from accidental omission.

`E_UNBOUND_TYPE_SLOT` rejects a reachable function or macro contract that uses
`*slot` without a binding in the selected entry. Bind a concrete nominal type
with `calcit config set-type-slot :slot namespace/definition`. When the entry
deliberately opts out of static checking at that boundary, bind `:dynamic`
explicitly; it remains visible to `analyze weak-types` and quality baselines but
is no longer confused with an omitted configuration. Compatibility mode keeps
the existing warning/inventory behavior while projects migrate.

`--strict-types` also rejects hand-written runtime Struct index operations such
as `&struct:nth` without matching nominal type evidence. Use `(:field value)` with a concrete nominal Struct; generic
helpers need a typed trait/accessor or a deliberately open Map boundary.

For a focused, machine-readable inventory that excludes unrelated type and FFI warnings, use the dedicated analysis command:

```bash
calcit analyze dynamic-methods
calcit analyze dynamic-methods --summary-only --format json
calcit analyze dynamic-methods --max 0
calcit analyze dynamic-methods --deps
```

The default scope contains project namespaces only. `--deps` includes reachable dependency namespaces, and `--max` returns a non-zero status when the finding count exceeds the reviewed limit.

### Macro Expansion Metrics (--macro-metrics)

Use opt-in macro metrics to profile compile, check, and hot-reload work without
changing normal CLI output:

```bash
cargo build --release --bin calcit
target/release/calcit --macro-metrics --check-only calcit/test.cirru
target/release/calcit --macro-metrics --check-only /path/to/respo/calcit.cirru js
```

The CLI writes one `macro-expansion-metrics: {...}` JSON record to stderr when
it exits. Timing fields use nanoseconds. Per-macro and total evaluator and
post-preprocess times are exclusive: when a nested macro starts, its parent's
timer pauses, so totals do not double-count recursive expansion work.

The report records general-evaluator fallbacks, cache misses, miss reasons, and
bypass reasons. Watch mode keeps a conservative raw-expansion cache for macros
with strict signatures and no compile-time capabilities. Entries are scoped to
stable source call sites and validate the macro identity, signature, exact input
syntax (including locations), and gensym sequence before reuse. Legacy,
effectful, runtime-evaluator, unstable-call-site, and non-watch calls report an
explicit bypass reason. `cacheInvalidations` separates changed macro definitions,
signatures, inputs, and gensym sequences; a cache hit skips macro evaluation but
still preprocesses and type-checks the emitted expansion.

The cache deliberately targets repeated preprocessing during hot reload. A
normal once-mode build does not populate it, avoiding cold-build memory and
cloning overhead. It does not cache post-preprocess results yet, so helper/import/
type dependency invalidation for that higher-ceiling optimization remains future
work.

Reproducible release-mode baseline from 2026-08-25 (three warm runs, median):

| Project | Revision | Expansions | Evaluator | Post-preprocess | General fallback |
| --- | --- | ---: | ---: | ---: | ---: |
| Calcit test snapshot | `32883223` | 2,432 | 20.21 ms | 52.61 ms | 2,432 |
| Respo JS check | `be8141e` | 1,414 | 14.24 ms | 35.50 ms | 1,414 |

Calcit's highest post-preprocess costs in the median run were `assert=`
(21.66 ms), `let` (10.16 ms), `def` (5.39 ms), and `fn` (4.20 ms). Respo's
were `let` (11.53 ms), `def` (4.71 ms), `fn` (2.92 ms), and `cond` (1.87 ms).
These results prioritize common structural macros and post-expansion processing
for the typed Macro IR phase; they do not claim application runtime gains.

### Native Async FFI Metrics (--ffi-metrics)

Use opt-in native async FFI metrics when a server or integration test needs a
machine-readable backpressure and lifecycle sample:

```bash
calcit --ffi-metrics --entry server
```

On normal exit or bounded Ctrl-C shutdown, the CLI writes exactly one
`ffi-async-metrics: {...}` JSON record to stderr. Business stdout remains
unchanged. The versioned report contains totals and stable module/method rows
for active, closing, and completed tasks; current queue depth, bytes, and oldest
age; enqueue/coalesce/queue-full/dequeue/purge counts; response deadline
timeouts; and cancellation requests, successes, and failures. Completed tasks
are folded into bounded module/method aggregates rather than retained one by
one. No payload is copied into or printed by the report.

使用 `--ffi-metrics` 可在服务或集成测试正常退出、或完成有界 Ctrl-C 关闭时，
向 stderr 输出唯一一条 `ffi-async-metrics: {...}` JSON 记录，不改变业务
stdout。报告按 module/method 汇总当前与已完成 task、queue backlog、oldest
age、enqueue/coalesce/queue-full/dequeue/purge、response deadline timeout 与
cancel 请求/成功/失败；已完成 task 只进入有界聚合，不逐项长期保留，也不会
复制或打印业务 payload。

### Hot Reloading Configuration

**--init-fn**: Override the main entry function:

```bash
calcit --init-fn app.main/start!
```

**--reload-fn**: Specify function called after code reload:

```bash
calcit --reload-fn app.main/on-reload!
```

**--reload-libs**: Force reload library data during hot reload (normally cached):

```bash
calcit --reload-libs
```

### Config Entry (--entry)

Use a specific entry from `calcit.cirru`. Without this option Calcit selects `entries.default`; the selected entry's `:mode` chooses native execution or JS emission:

```bash
calcit --entry test
calcit --entry production
```

```cirru.no-check
:entries $ {}
  :default $ {} (:mode :js) (:init-fn 'app.main/main!) (:reload-fn 'app.main/reload!)
  :test $ {} (:mode :native) (:init-fn 'app.test/main!) (:reload-fn 'app.test/reload!)
```

The explicit `js` subcommand remains a compatibility/debug override. Prefer configuring `:mode` so the same entry is invoked consistently by developers and CI.

### Asset Watching (--watch-dir)

Watch additional directories for changes (e.g., assets, styles):

```bash
calcit --watch-dir assets/
calcit --watch-dir styles/ --watch-dir images/
```

## Common Usage Patterns

```bash
# Development with watch mode
calcit -w --reload-fn app.main/reload!

# Production build
calcit js --emit-path dist/

# JS watch mode
calcit js -w --emit-path dist/

# Testing single run
calcit --init-fn app.test/run-tests!

# Debug mode with full stack traces
calcit --reload-libs

# CI/CD environment
calcit --disable-stack
```

`calcit ir` emits an internal representation for compiler debugging. Ordinary application development and CI do not need it; inspect `calcit ir --help` only when debugging that layer.

## Markdown code checking

Use `docs check-md` to validate fenced code blocks in markdown files:

```bash
calcit docs check-md README.md
```

This defaults to `calcit.cirru` as the eval entry. If your project uses a different snapshot filename, pass it explicitly with `--entry`:

```bash
calcit docs check-md README.md --entry calcit.cirru
```

Load module dependencies with repeatable `--dep` options:

```bash
calcit docs check-md README.md --dep ./ --dep ~/.config/calcit/modules/memof/
```

Format the same fenced Cirru blocks with `docs format-md`. It preserves all
Markdown outside recognized `cirru`, `cirru.no-run`, `cirru.no-check`,
`cirru.cli`, and `cirru.edn` fences, and writes through an atomic replacement:

```bash
calcit docs format-md README.md
```

Use `--check` in CI to reject non-canonical snippets without changing files:

```bash
calcit docs format-md README.md --check
```

Recommended block modes:

- `cirru`: run + preprocess + parse (preferred; executes injected snippet entry `app.main/main!`, not entry file `:init-fn`)
- `cirru.no-run`: preprocess + parse when runtime setup is unavailable
- `cirru.no-check`: parse only for illustrative snippets
- `cirru.edn`: not Calcit code — parse as EDN data, for schema/config snippets such as `CodeEntry :schema`/`:ffi` payloads

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
entry_for:
  - "calcit -w"
  - "calcit js -w"
  - "calcit --help"
  - "calcit --reload-fn"
---

# CLI Options

```bash
calcit --help
```

Quick note: `calcit edit format` rewrites the target snapshot using canonical serialization without guessing semantic changes. It normalizes legacy namespace records and top-level `:configs`, and rewrites legacy schema type tags such as `:string` and `:ref` to quoted symbols such as `'String` and `'Ref` only in known type positions. Ordinary tag data stays unchanged. It then emits recoverable stderr advisories for legacy filenames, legacy `:any`, and unresolved dynamic type debt. Use `calcit analyze weak-types` for exact paths and recommendations; format warnings do not turn the command into a type-quality gate.

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

For a focused, machine-readable inventory that excludes unrelated type and FFI warnings, use the dedicated analysis command:

```bash
calcit analyze dynamic-methods
calcit analyze dynamic-methods --summary-only --format json
calcit analyze dynamic-methods --max 0
calcit analyze dynamic-methods --deps
```

The default scope contains project namespaces only. `--deps` includes reachable dependency namespaces, and `--max` returns a non-zero status when the finding count exceeds the reviewed limit.

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

Use a specific entry from `calcit.cirru` (legacy filename: `compact.cirru`). Without this option Calcit selects `entries.default`; the selected entry's `:mode` chooses native execution or JS emission:

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
calcit docs check-md README.md --entry compact.cirru
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

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
  - "cr -w"
  - "cr js -w"
  - "cr ir -w"
  - "cr --help"
  - "cr --reload-fn"
---

# CLI Options

```bash
cr --help
```

Quick note: `cr edit format` rewrites the target snapshot using canonical serialization without changing semantics. It also normalizes legacy namespace entries that were previously serialized with `CodeEntry` into the current `NsEntry` shape.

## Detailed Option Descriptions

### Input File

```bash
# Run default compact.cirru
cr

# Run specific file
cr demos/compact.cirru
```

### Run Mode (default once)

By default, `cr` runs once and exits. Use `--watch` (`-w`) to enable watch mode:

```bash
cr --watch
cr -w demos/compact.cirru
```

### Error Stack Trace (--disable-stack)

Disables detailed stack traces in error messages, useful for cleaner output:

```bash
cr --disable-stack
```

### JS Codegen Options

**--skip-arity-check**: When generating JavaScript, skip arity checking (use cautiously):

```bash
cr js --skip-arity-check
```

**--emit-path**: Specify output directory for generated JavaScript:

```bash
cr js --emit-path dist/
```

### Dynamic Method Warnings (--warn-dyn-method)

Warn when dynamic method dispatch cannot be specialized at preprocess time, and surface related trait-attachment diagnostics:

```bash
cr --warn-dyn-method
```

### Hot Reloading Configuration

**--init-fn**: Override the main entry function:

```bash
cr --init-fn app.main/start!
```

**--reload-fn**: Specify function called after code reload:

```bash
cr --reload-fn app.main/on-reload!
```

**--reload-libs**: Force reload library data during hot reload (normally cached):

```bash
cr --reload-libs
```

### Config Entry (--entry)

Use specific config entry from `compact.cirru`:

```bash
cr --entry test
cr --entry production
```

### Asset Watching (--watch-dir)

Watch additional directories for changes (e.g., assets, styles):

```bash
cr --watch-dir assets/
cr --watch-dir styles/ --watch-dir images/
```

## Common Usage Patterns

```bash
# Development with watch mode
cr -w --reload-fn app.main/reload!

# Production build
cr js --emit-path dist/

# JS watch mode
cr js -w --emit-path dist/

# IR watch mode
cr ir -w

# WASM codegen (experimental, numeric subset only)
cr-wasm
cr-wasm demos/wasm-demo.cirru

# Testing single run
cr --init-fn app.test/run-tests!

# Debug mode with full stack traces
cr --reload-libs

# CI/CD environment
cr --disable-stack
```

### WASM Codegen

Generate WAT (WebAssembly Text format) for pure numeric functions:

```bash
cr-wasm                          # compile init namespace to wasm binary
cr-wasm demos/wasm-demo.cirru    # compile specific file
```

Output is written to `js-out/program.wat`. Only a subset of Calcit is supported (numbers, `if`, `let`, arithmetic, comparisons, `recur`, function calls). Unsupported functions are skipped with a warning.

See [WASM Codegen](../wasm-codegen.md) for full details and supported features.

## Markdown code checking

Use `docs check-md` to validate fenced code blocks in markdown files:

```bash
cr docs check-md README.md
```

Load module dependencies with repeatable `--dep` options:

```bash
cr docs check-md README.md --dep ./ --dep ~/.config/calcit/modules/memof/
```

Recommended block modes:

- `cirru`: run + preprocess + parse (preferred)
- `cirru.no-run`: preprocess + parse when runtime setup is unavailable
- `cirru.no-check`: parse only for illustrative snippets

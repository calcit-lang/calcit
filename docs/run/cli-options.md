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
  - "cr --help"
  - "cr --reload-fn"
---

# CLI Options

```bash
cr --help
```

Quick note: `cr edit format` rewrites the target snapshot using canonical serialization without guessing semantic changes. It normalizes legacy namespace records and top-level `:configs`, then emits recoverable stderr advisories for legacy filenames, legacy `:any`, and unresolved dynamic type debt. Use `cr analyze weak-types` for exact paths and recommendations; format warnings do not turn the command into a type-quality gate.

## Detailed Option Descriptions

### Input File

```bash
# Run default calcit.cirru
cr

# Run specific file
cr calcit.cirru
```

### Run Mode (default once)

By default, `cr` runs once and exits. Use `--watch` (`-w`) to enable watch mode:

```bash
cr --watch
cr -w calcit.cirru
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

Use a specific entry from `calcit.cirru` (legacy filename: `compact.cirru`). Without this option Calcit selects `entries.default`; the selected entry's `:mode` chooses native execution or JS emission:

```bash
cr --entry test
cr --entry production
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

# Testing single run
cr --init-fn app.test/run-tests!

# Debug mode with full stack traces
cr --reload-libs

# CI/CD environment
cr --disable-stack
```

`cr ir` emits an internal representation for compiler debugging. Ordinary application development and CI do not need it; inspect `cr ir --help` only when debugging that layer.

## Markdown code checking

Use `docs check-md` to validate fenced code blocks in markdown files:

```bash
cr docs check-md README.md
```

This defaults to `calcit.cirru` as the eval entry. If your project uses a different snapshot filename, pass it explicitly with `--entry`:

```bash
cr docs check-md README.md --entry compact.cirru
```

Load module dependencies with repeatable `--dep` options:

```bash
cr docs check-md README.md --dep ./ --dep ~/.config/calcit/modules/memof/
```

Recommended block modes:

- `cirru`: run + preprocess + parse (preferred; executes injected snippet entry `app.main/main!`, not entry file `:init-fn`)
- `cirru.no-run`: preprocess + parse when runtime setup is unavailable
- `cirru.no-check`: parse only for illustrative snippets

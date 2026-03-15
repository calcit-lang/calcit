# CLI Options

```bash
Usage: cr [<input>] [-1] [-w] [--disable-stack] [--skip-arity-check] [--warn-dyn-method] [--emit-path <emit-path>] [--init-fn <init-fn>] [--reload-fn <reload-fn>] [--entry <entry>] [--reload-libs] [--watch-dir <watch-dir>] [<command>] [<args>]

Top-level command.

Positional Arguments:
  input             input source file, defaults to "compact.cirru"

Options:
  -1, --once        run once and quit (compatibility option)
  -w, --watch       watch files and rerun/rebuild on changes
  --disable-stack   disable stack trace for errors
  --skip-arity-check
                    skip arity check in js codegen
  --warn-dyn-method
                    warn on dynamic method dispatch and trait-attachment diagnostics
  --emit-path       entry file path, defaults to "js-out/"
  --init-fn         specify `init_fn` which is main function
  --reload-fn       specify `reload_fn` which is called after hot reload
  --entry           specify with config entry
  --reload-libs     force reloading libs data during code reload
  --watch-dir       specify a path to watch assets changes
  --help            display usage information

Commands:
  js                emit JavaScript rather than interpreting
  ir                emit Cirru EDN representation of program to program-ir.cirru
  eval              run program
  analyze           analyze code structure (call-graph, count-calls, check-examples)
  query             query project information (namespaces, definitions, configs)
  docs              documentation tools (guidebook)
  cirru             Cirru syntax tools (parse, format, edn)
  libs              fetch available Calcit libraries from registry
  edit              edit project code (definitions, namespaces, modules, configs)
  tree              fine-grained code tree operations (view and modify AST nodes)
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

### Run Once (--once / -1)

By default, `cr` runs once and exits. Use `--watch` (`-w`) to enable watch mode:

```bash
cr --watch
cr -w demos/compact.cirru
```

`--once` is still available for compatibility:

```bash
cr --once
cr -1  # shorthand
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

# Testing single run
cr --once --init-fn app.test/run-tests!

# Debug mode with full stack traces
cr --reload-libs

# CI/CD environment
cr --once --disable-stack
```

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

---
title: "Bundle Mode"
scope: "core"
kind: "guide"
category: "run"
aliases:
  - "bundle mode"
  - "single file deployment"
  - "bundle"
---
# Bundle Mode

Calcit programs are primarily designed to be written using the [calcit-editor](http://github.com/calcit-lang/editor), a structural editor.

You can also try short code snippets in eval mode:

```bash
cr eval "+ 1 2"
# => 3
```

If you prefer to write Calcit code without the calcit-editor, that's possible too. See the example in [minimal-calcit](https://github.com/calcit-lang/minimal-calcit).

Calcit code can be written using indentation-based syntax. This means you don't need to match parentheses as in Clojure, but you must pay close attention to indentation.

Use a `calcit.cirru` file (legacy filename: `compact.cirru`) with the `cr` command to run the program.

For projects that still keep one namespace per indentation-based `.cirru` source file, the repository includes a one-shot Calcit bundler example:

```bash
BUNDLE_SRC=src \
BUNDLE_CONFIG=deps.cirru \
BUNDLE_OUT=calcit.cirru \
cr /path/to/calcit/calcit/scripts/bundle-calcit.cirru
```

The script recursively reads `.cirru` files, validates their `ns` and definition forms, and writes a runnable snapshot. It intentionally does not implement the retired watch or incremental-file behavior.

To synchronize a legacy runtime `compact.cirru` back into a detailed `calcit.cirru`, use the companion script. It requires locally installed `bisection-key` version `0.0.18` or later:

```bash
SYNC_COMPACT=compact.cirru \
SYNC_CALCIT=calcit.cirru \
cr /path/to/calcit/calcit/scripts/sync-calcit.cirru
```

The sync script preserves metadata for unchanged code and regenerates detailed lexical keys only for changed code.

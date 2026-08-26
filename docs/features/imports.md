---
title: "Imports"
summary: "命名空间导入语法：:require、:refer、:as、:default，以及 calcit edit add-import/imports 命令管理"
scope: "core"
kind: "reference"
category: "features"
aliases:
  - "namespace imports"
  - "require"
  - "import module"
  - "import namespace"
  - "add-import"
  - "refer"
  - "module imports"
  - "add import"
  - "edit imports"
entry_for:
  - "calcit edit add-import"
  - "calcit edit imports"
  - "ns :require"
---

# Imports

Calcit loads namespaces from `calcit.cirru`. Dependencies are installed into the project module view under `.calcit/modules/`.

## Quick Recipes

- **Alias**: `:require (app.lib :as lib)`
- **Refer**: `:require (app.lib :refer $ f1 f2)`
- **Core**: `calcit.core` is auto-imported
- **CLI Add**: `calcit edit add-import app.main --code 'quote (app.lib :refer $ f1)'`

## The `ns` Form

Every source file declares its namespace at the top with `ns`:

```cirru.no-check
ns app.demo
  :require
    app.lib :as lib
    app.lib :refer $ f1 f2
    app.util :refer $ helper
```

The `:require` block accepts two kinds of rules:

| Form                        | Effect                                              |
| --------------------------- | --------------------------------------------------- |
| `mod.ns :as alias`          | Imports namespace as `alias`; access via `alias/fn` |
| `mod.ns :refer $ sym1 sym2` | Imports symbols directly into scope                 |

## Aliased Import

Use `:as` to import an entire namespace under a local alias:

```cirru.no-check
ns app.main
  :require
    app.model :as model
    app.util :as util

; Then use as:
; model/make-user
; util/format-date
```

## Direct Symbol Import

Use `:refer` to bring specific names into the current namespace:

```cirru.no-check
ns app.main
  :require
    app.math :refer $ add subtract multiply
    app.string :refer $ capitalize trim-whitespace
```

## `calcit.core` — Auto-Imported

All standard library functions (`map`, `filter`, `reduce`, `+`, `println`, `defn`, `let`, etc.) come from `calcit.core` and are available automatically without an explicit import. You do **not** need to require `calcit.core`.

## JavaScript Interop Imports

When compiling to JavaScript, Calcit generates ES module import syntax. The NS form supports additional rules for JS:

```cirru.no-check
ns app.demo
  :require
    ; Regular Calcit module
    app.lib :as lib

    ; NPM package with default export
    |chalk :default chalk

    ; NPM package with named exports
    |path :refer $ join dirname
```

Generated JS output:

```js
import * as $app_DOT_lib from "./app.lib.mjs";
import chalk from "chalk";
import { join, dirname } from "path";
```

Note the `|` prefix on npm package names — this indicates a string literal (the module specifier) vs a Calcit namespace path.

## Validation and Dead-Code Analysis

Import rules are validated both when a snapshot is loaded and before `calcit edit add-import` or `calcit edit imports` saves it. A rule must:

- contain exactly three items;
- use one of `:as`, `:refer`, or `:default`;
- use a namespace/module leaf and symbol bindings in the positions required by that rule.

Malformed rules report their rule index; errors found while loading also identify the namespace. They no longer fail with an internal panic. Import editing is atomic: validation errors leave the snapshot unchanged.

Duplicate local aliases or referred symbols are recoverable. Calcit writes a warning to stderr and continues with the later rule taking precedence. This preserves existing program execution while making accidental shadowing visible.

To find definitions that are not reachable from the configured entry function, run:

```bash
calcit calcit.cirru analyze call-graph --show-unused
```

This is an entry-relative static analysis, not a proof that every reported definition is dead. Public functions, alternate entry points, and definitions invoked externally can appear in the report. It currently reports unreachable definitions rather than unused import declarations.

## Avoiding Circular Imports

Circular dependencies (A imports B, B imports A) will cause a compilation error. Structure your code with:

- Core data types and pure functions in low-level namespaces
- Side-effectful and orchestration code at higher levels

## Using `calcit edit` for Import Management

The `calcit edit` CLI commands help manage imports safely:

```bash
# Add a new import to a namespace
calcit app.cirru edit add-import app.demo --code 'quote (app.util :refer $ helper)'

# Override an existing import (same source namespace)
calcit app.cirru edit add-import app.demo --code 'quote (app.util :refer $ helper new-fn)' --overwrite
```

See `calcit edit --help` for all available operations.

## Checking Imports

Use `calcit docs search` to look up what's available in a namespace before importing:

```bash
calcit app.cirru docs search my-function
```

or query the examples for a specific definition:

```bash
calcit app.cirru query examples calcit.core/map
```

# Imports

Calcit loads namespaces from `compact.cirru` (the compiled representation of source files). Dependencies are tracked via `~/.config/calcit/modules/`.

## Quick Recipes

- **Alias**: `:require (app.lib :as lib)`
- **Refer**: `:require (app.lib :refer $ f1 f2)`
- **Core**: `calcit.core` is auto-imported
- **CLI Add**: `cr edit add-import app.main -e 'app.lib :refer $ f1'`

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

## Avoiding Circular Imports

Circular dependencies (A imports B, B imports A) will cause a compilation error. Structure your code with:

- Core data types and pure functions in low-level namespaces
- Side-effectful and orchestration code at higher levels

## Using `cr edit` for Import Management

The `cr edit` CLI commands help manage imports safely:

```bash
# Add a new import to a namespace
cr app.cirru edit add-import app.demo -e 'app.util :refer $ helper'

# Override an existing import (same source namespace)
cr app.cirru edit add-import app.demo -e 'app.util :refer $ helper new-fn' -o
```

See `cr edit --help` for all available operations.

## Checking Imports

Use `cr docs search` to look up what's available in a namespace before importing:

```bash
cr app.cirru docs search my-function
```

or query the examples for a specific definition:

```bash
cr app.cirru query examples calcit.core/map
```

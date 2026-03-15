# Querying Definitions

Calcit provides a powerful `query` subcommand to inspect code, find definitions, and analyze usages directly from the command line.

## Core Query Commands

### List Namespaces (`ns`)

```bash
# List all loaded namespaces
cr query ns

# Show definitions in a specific namespace
cr query ns calcit.core
```

### Read Code (`def`)

```bash
# Show full source code of a definition
cr query def calcit.core/assoc
```

### Peek Signature (`peek`)

```bash
# Show documentation and examples without the full body
cr query peek calcit.core/map
```

### Check Examples (`examples`)

```bash
# Extract only the examples section
cr query examples calcit.core/let
```

### Find Symbol (`find`)

```bash
# Search for a symbol across ALL loaded namespaces
cr query find assoc
```

### Analyze Usages (`usages`)

```bash
# Find where a specific definition is used
cr query usages app.main/main!
```

### Search Text (`search`)

```bash
# Search for raw text (leaf values) across project
cr query search hello

# Limit to one definition
cr query search hello -f app.main/main!
```

### Search Expressions (`search-expr`)

```bash
# Search structural expressions (Cirru pattern)
cr query search-expr "fn (x)"

# Limit to one definition
cr query search-expr "fn (x)" -f app.main/main!
```

## Quick Recipes (for fast locating)

### Locate a symbol and jump to definition

```bash
cr query find assoc
cr query def calcit.core/assoc
```

### Locate all call sites before refactor

```bash
cr query usages app.main/main!
```

### Locate by text when you only remember a fragment

```bash
cr query search "reload"
```

## Runtime Code Inspection

You can also use built-in functions to inspect live data and definitions:

```cirru
let
    Point $ defstruct Point (:x :number) (:y :number)
    p (%{} Point (:x 1) (:y 2))
  do
    ; Get all methods/traits implemented by a value
    println $ &methods-of p
    ; Get tag name of a record or enum
    println $ &record:get-name p
    ; Describe any value's internal type
    println $ &inspect-type p
```

### Getting Help

Use `cr query --help` for the full list of available query subcommands.

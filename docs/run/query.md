---
title: "Querying Definitions"summary: "使用 cr query defs/def/search/find/usages/search-expr 查找和浏览定义"scope: "core"
kind: "reference"
category: "run"
aliases:
  - "query defs"
  - "query ns"
  - "query def"
  - "usages"
  - "find symbol"
  - "search-expr"
  - "search expr"
entry_for:
  - "cr query ns"
  - "cr query defs"
  - "cr query def"
  - "cr query find"
  - "cr query usages"
  - "cr query search-expr"
---

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

# Builtin helpers without snapshot source still return metadata
cr query def calcit.core/to-js-data
```

For source-backed definitions, `query def` prints the stored Cirru body. For special builtin helpers such as `calcit.core/to-js-data`, it falls back to builtin metadata (doc, schema, examples count) even when no snapshot source exists.

### Peek Signature (`peek`)

```bash
# Show documentation and examples without the full body
cr query peek calcit.core/map
```

### Check Examples (`examples`)

```bash
# Extract only the examples section
cr query examples calcit.core/let

# Builtin helpers can also expose curated examples when available
cr query examples calcit.core/to-js-data
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
cr query search hello --filter app.main/main!
```

### Search Expressions (`search-expr`)

```bash
# Search structural expressions (Cirru pattern)
cr query search-expr "fn (x)"

# Limit to one definition
cr query search-expr "fn (x)" --filter app.main/main!
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
    ; "Get all methods/traits implemented by a value"
    println $ &methods-of p
    ; 'Get tag name of a record or enum'
    println $ &record:get-name p
    ; "Describe any value's internal type"
    println $ &inspect-type p
```

### Getting Help

Use `cr query --help` for the full list of available query subcommands.

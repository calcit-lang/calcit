---
title: "Documentation & Libraries"
scope: "core"
kind: "reference"
category: "run"
aliases:
  - "docs"
  - "libs"
  - "read-lines"
  - "scan-md"
  - "docs read"
entry_for:
  - "calcit docs search"
  - "calcit docs read"
  - "calcit docs read-lines"
  - "calcit docs remote-libs scan-md"
---

# Documentation & Libraries

Calcit includes built-in commands to navigate the language guidebook, installed module docs, and discover community libraries.

## Guidebook Access (`docs`)

The `docs` subcommand uses a progressive disclosure flow: scope -> file -> sections -> content. Scope is either built-in `calcit` docs or one installed module selected with `--module`.

### Reading Chapters

```bash
# List available documentation scopes first
calcit docs scopes

# List all chapters in the built-in calcit guidebook
calcit docs list

# List docs from one installed module
calcit docs list --module respo.calcit

# Read the local Agent guide (frontmatter, if present, is hidden automatically)
calcit docs agents

# Read a specific file (fuzzy matching supported)
calcit docs read run.md

# Read by title/alias instead of exact filename
calcit docs read search-replace
calcit docs read "CLI Code Editing"

# List headings in a file (best first step before narrowing)
calcit docs sections run.md

# Jump by heading keyword(s)
calcit docs read run.md quick start

# Search for keywords across all chapters
calcit docs search "polymorphism"

# Search installed module docs only
calcit docs search render --module respo.calcit

# Read a specific installed module doc directly
calcit docs read Respo-Agent --module respo.calcit
```

### Advanced Navigation (`read`)

`calcit docs read` supports fuzzy heading matching to jump straight to a section, while `calcit docs sections` is the dedicated heading discovery step:

```bash
# Display the "Quick start" section of run.md
calcit docs read run.md "Quick start"

# Exclude subheadings from the output
calcit docs read run.md "Quick start" --no-subheadings
```

### Precision Reading (`read-lines`)

Use `read-lines` for large files where you need a specific range:

```bash
# Read 50 lines starting from line 100 of common-patterns.md
calcit docs read-lines common-patterns.md --start 100 --lines 50

# Resolve by alias/title first, then read a specific range
calcit docs read-lines search-replace --start 48 --lines 8

# Read a module document by title/alias with the same resolver
calcit docs read-lines Respo-Agent --module respo.calcit --start 1 --lines 8

# Discover headings first, then narrow
calcit docs sections query.md
calcit docs read query.md usages
```

### Pattern 2: Search globally, then open exact chapter

```bash
calcit docs search trait
calcit docs read traits.md
```

### Pattern 3: Search by documentation scope

```bash
# Search one installed module directly
calcit docs search defstyle --module respo.calcit

# Search module Agent/docs together and let ranking pick the better hit
calcit docs search render --module respo.calcit

# Read one module document directly with the same resolver
calcit docs read Respo-Agent --module respo.calcit
```

## Library Discovery (`docs remote-libs`)

Use `calcit docs remote-libs` for package registry discovery and package README retrieval. If you are querying calcit/module docs content, stay under `calcit docs ...`.

### Searching Registry

```bash
# Search for libraries related to "web"
calcit docs remote-libs search web
```

### Reading Readmes

You can read the documentation of any official library, even if not installed locally:

```bash
# Show README of 'respo' module
calcit docs remote-libs readme respo

# Read a specific markdown file inside package
calcit docs remote-libs readme respo --file Skills.md
```

### Low-Level File Listing

```bash
# Prefer `calcit docs list --module memof` for installed module docs
calcit docs list --module memof

# `calcit docs remote-libs scan-md` remains available as a low-level compatibility shortcut
calcit docs remote-libs scan-md memof
```

## Collaborative validation (`check-md`)

`docs check-md` is used to verify that code blocks in your markdown documentation are correct and runnable:

```bash
calcit docs check-md README.md
```

By default this uses `calcit.cirru` as the entry file. For projects using another snapshot filename, pass `--entry <snapshot-file>` explicitly.

It supports specific block types:

- `cirru`: Run and validate.
- `cirru.no-run`: Validate syntax and preprocessing without running.
- `cirru.no-check`: Skip checking (illustrative).

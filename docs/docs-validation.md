---
title: "Documentation Retrieval Validation"
scope: "core"
kind: "reference"
category: "docs"
aliases:
  - "docs validation"
  - "search validation"
  - "retrieval regression"
entry_for:
  - "cr docs search"
  - "cr docs read"
  - "cr docs read-lines"
---

# Documentation Retrieval Validation

This page contains executable validation cases for `cr docs search`, `cr docs read`, and `cr docs read-lines`.

Use it when changing:

- frontmatter fields and conventions
- ranking weights
- resolver behavior
- scope/module routing

## Core Search Checks

```bash
cr docs search polymorphism
cr docs search edit-tree -f run.md
cr docs search search-replace
cr docs search watch mode
cr docs search calcit.cirru
```

## Summary Mode Checks

```bash
# --summary should show only title + summary, no content snippets
cr docs search eval --summary
cr docs search cirru --summary

# Should still work when no summary field exists
cr docs search scopes --summary

# Summary mode with module filter
cr docs search render --module respo.calcit --summary
```

## Hub Marking Checks

```bash
# Results should show [Hub] marker on hub-type documents
cr docs search features
cr docs search calcit.cirru
```

## Module Search Checks

```bash
cr docs search render --module respo.calcit
cr docs search clear-cache --module respo.calcit
cr docs search defstyle --module respo.calcit
cr docs search hook --module respo.calcit
```

## Ranking Checks

```bash
# A topical page should beat an examples/spec page that only mentions the term
cr docs search polymorphism

# Alias-only queries should still find the right page
cr docs search search-replace
cr docs search hot reload

# Command-oriented phrases should point at the operational guide
cr docs search "cr eval"
cr docs search "add-import"

# Filename/path hits should stay useful when several pages mention the same term
cr docs search traits
cr docs search docs
```

## Agents and Module Checks

```bash
# Agents frontmatter should not leak into output
cr docs agents

# Module Agents and module docs should both participate in ranking
cr docs search render --module respo.calcit
cr docs search clear-cache --module respo.calcit
```

## Read Checks

```bash
# Metadata should not leak into read output
cr docs read polymorphism.md
cr docs read edit-tree.md search-replace

# Resolver should work with aliases, titles, and command phrases
cr docs read search-replace
cr docs read "CLI Code Editing"
cr docs read "cr eval --dep"
cr docs read "cr edit add-import"
cr docs read "query ns"
cr docs read "reload fn"
cr docs read "indentation based syntax"
```

## Read-Lines Checks

```bash
cr docs read-lines search-replace --start 48 --lines 8
cr docs read-lines Respo-Agent --module respo.calcit --start 1 --lines 8
```

## Update Rule

When behavior changes:

1. Update [docs/docs-indexing.md](docs/docs-indexing.md) if the rule changed.
2. Update this file if the expected observable behavior changed.
3. Re-run the relevant commands above.

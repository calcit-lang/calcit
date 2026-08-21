---
title: "Documentation Retrieval Validation"
summary: "通过可执行案例验证 calcit docs 的搜索、摘要、章节解析和模块路由"
scope: "core"
kind: "reference"
category: "docs"
aliases:
  - "docs validation"
  - "search validation"
  - "retrieval regression"
entry_for:
  - "calcit docs search"
  - "calcit docs read"
  - "calcit docs read-lines"
id: core/docs/validation
parent: core/docs
related:
  - core/docs/indexing
requires:
  - core/docs/indexing
---

# Documentation Retrieval Validation

This page contains executable validation cases for `calcit docs search`, `calcit docs read`, and `calcit docs read-lines`.

Use it when changing:

- frontmatter fields and conventions
- ranking weights
- resolver behavior
- scope/module routing

## New File Checks

```bash
# New split files should be searchable by alias and content
calcit docs search quick-start
calcit docs search project-structure
calcit docs search structural-strategies
calcit docs search debugging

# Summary mode on new files
calcit docs search "project structure" --summary
calcit docs search debugging --summary
calcit docs search structural --summary
```

```bash
calcit docs search polymorphism
calcit docs search edit-tree --filename run.md
calcit docs search search-replace
calcit docs search watch mode
calcit docs search calcit.cirru
```

## Summary Mode Checks

```bash
# --summary should show only title + summary, no content snippets
calcit docs search eval --summary
calcit docs search cirru --summary

# Should still work when no summary field exists
calcit docs search scopes --summary

# Summary mode with module filter
calcit docs search render --module respo.calcit --summary
```

## Hub Marking Checks

```bash
# Results should show [Hub] marker on hub-type documents
calcit docs search features
calcit docs search calcit.cirru
```

## Module Search Checks

```bash
calcit docs search render --module respo.calcit
calcit docs search clear-cache --module respo.calcit
calcit docs search defstyle --module respo.calcit
calcit docs search hook --module respo.calcit
```

## Ranking Checks

```bash
# A topical page should beat an examples/spec page that only mentions the term
calcit docs search polymorphism

# Alias-only queries should still find the right page
calcit docs search search-replace
calcit docs search hot reload

# Command-oriented phrases should point at the operational guide
calcit docs search "calcit eval"
calcit docs search "add-import"

# Filename/path hits should stay useful when several pages mention the same term
calcit docs search traits
calcit docs search docs
```

## Knowledge Navigation Checks

```bash
# A definition name should resolve to the structure-level entry document.
calcit docs search "calcit.core/nth" --summary
calcit docs search "calcit.core/append" --summary

# The structured relationship section should be addressable by heading.
calcit docs read "Documentation Indexing Spec" "Knowledge graph fields"

# A structure page should expose its operation-oriented sections.
calcit docs read "List" "Quick Recipes"

# Build and traverse the structured knowledge graph.
calcit docs graph build
calcit docs graph check
calcit docs graph children core/features
calcit docs graph related core/features/list
calcit docs graph path core/features/list core/run/edit-tree
calcit docs graph explain calcit.core/nth
calcit docs graph explain calcit.core/nth --full
calcit docs graph missing --ns calcit.core --limit 20
calcit docs graph orphans
```

## Agents and Module Checks

```bash
# Agents frontmatter should not leak into output
calcit docs agents

# Module Agents and module docs should both participate in ranking
calcit docs search render --module respo.calcit
calcit docs search clear-cache --module respo.calcit
```

## Read Checks

```bash
# Metadata should not leak into read output
calcit docs read polymorphism.md
calcit docs read edit-tree.md search-replace

# Resolver should work with aliases, titles, and command phrases
calcit docs read search-replace
calcit docs read "CLI Code Editing"
calcit docs read "calcit eval --dep"
calcit docs read "calcit edit add-import"
calcit docs read "query ns"
calcit docs read "reload fn"
calcit docs read "indentation based syntax"
```

## Read-Lines Checks

```bash
calcit docs read-lines search-replace --start 48 --lines 8
calcit docs read-lines Respo-Agent --module respo.calcit --start 1 --lines 8
```

## Update Rule

When behavior changes:

1. Update [docs/docs-indexing.md](docs/docs-indexing.md) if the rule changed.
2. Update this file if the expected observable behavior changed.
3. Re-run the relevant commands above.

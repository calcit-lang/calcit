---
title: "Documentation Indexing Spec"
scope: "core"
kind: "spec"
category: "docs"
aliases:
  - "frontmatter"
  - "docs metadata"
  - "search indexing"
entry_for:
  - "cr docs search"
  - "cr docs read"
  - "cr docs read-lines"
---

# Documentation Indexing Spec

This page defines the documentation layout expected by `cr docs`.

The goal is to keep docs git-friendly and readable while still giving the CLI enough structure for fast lookup.

## Scope Layout

There are 2 documentation scopes:

- `core`:
  - `~/.config/calcit/Agents.md`
  - `~/.config/calcit/docs/`
- `module`:
  - `~/.config/calcit/modules/<module>/Agents.md`
  - `~/.config/calcit/modules/<module>/docs/`

`core` docs explain the Calcit toolchain itself.
`module` docs explain APIs and workflows that belong to a specific installed module.

## Responsibilities

Core docs should cover:

- Cirru syntax
- CLI behavior
- query/edit/run workflows
- language/runtime semantics

Module docs should cover:

- module APIs
- module-specific workflows
- module-specific conventions
- domain examples that do not belong in Calcit core docs

## Minimal File Rules

For module-aware lookup to work well:

- every module should provide `Agents.md` as the module entry page when practical
- module markdown files should live under `docs/`
- file names should stay stable and descriptive
- headings should prefer task-oriented wording over abstract labels

## Frontmatter Schema

Each markdown file under `docs/` should provide a small frontmatter block.

The same schema is also supported for:

- core `Agents.md`
- module `Agents.md`
- module markdown files under `~/.config/calcit/modules/<module>/docs/`

Required fields:

- `title`: user-facing page title
- `scope`: `core` or `module`
- `kind`: one of `hub`, `guide`, `reference`, `spec`, `agent`
- `category`: one value from the category registry below

### Category Registry

Use these categories for core docs unless there is a strong reason not to:

- `run`: CLI execution, eval, query, edit-tree, docs commands, upgrade workflow
- `features`: language features such as traits, macros, records, tuples, enums, collections
- `installation`: installation, modules directory, CI setup, runtime bindings
- `data`: data literals and data structures, such as strings, EDN, persistent collections
- `intro`: onboarding and conceptual overview pages
- `syntax`: Cirru syntax and `calcit.cirru`/indentation-oriented structure docs
- `tools`: deprecated or auxiliary tooling docs, such as editor-oriented pages
- `ecosystem`: library landscape and surrounding projects
- `reference`: cross-cutting lookup pages such as cheatsheets
- `docs`: documentation system specs and indexing conventions themselves

Category selection rules:

- prefer an existing category over inventing a new one
- choose by the page's primary retrieval intent, not by one incidental section
- use `reference` only for genuinely cross-cutting lookup material; do not use it as a fallback for every technical page
- use `docs` only for pages about the documentation system itself
- introduce a new category only when several pages clearly form a stable family

Optional fields:

- `aliases`: extra phrases that users are likely to search for
- `entry_for`: commands, APIs, or task phrases that should point to this page

Recommended shape:

```yaml
---
title: "CLI Code Editing"
scope: "core"
kind: "guide"
category: "run"
aliases:
  - "edit tree"
  - "search-replace"
  - "imports"
entry_for:
  - "cr tree search-replace"
  - "cr edit add-import"
---
```

Notes:

- frontmatter should stay short and retrieval-oriented
- `aliases` should use phrases people actually type, not every possible synonym
- `entry_for` is best for commands, APIs, and exact task wording
- `scope` is authored explicitly even if it can be inferred from directory layout
- `Agents.md` can use the same schema; `cr docs agents` will strip frontmatter before rendering
- unknown `category` values are rejected during doc loading so the registry stays stable

Examples:

- `docs/api.md`
- `docs/guide/hot-swapping.md`
- `docs/apis/render!.md`

## Search Behavior

`cr docs search` defaults to built-in `calcit` docs and supports narrowing to one installed module:

```bash
cr docs search render --module respo.calcit
```

## Read Behavior

`cr docs list` lists files in the current doc scope.

`cr docs sections <file>` lists headings in one file.

`cr docs read` uses the same document resolver style as `search`.

`cr docs read-lines` uses the same resolver too.

It can resolve a page by:

- filename
- relative path fragment
- frontmatter `title`
- frontmatter `aliases`
- frontmatter `entry_for`

It also supports narrowing to one module with `--module <name>`.

Examples:

```bash
cr docs scopes
cr docs list
cr docs sections search-replace
cr docs read search-replace
cr docs read "CLI Code Editing"
cr docs read Respo-Agent --module respo.calcit
cr docs read-lines search-replace --start 48 --lines 8
cr docs read-lines Respo-Agent --module respo.calcit --start 1 --lines 8
```

## Recommended Authoring Style

To improve CLI recall without introducing heavy metadata:

- keep one topic per file when possible
- use clear headings such as `Quick start`, `Hot swapping`, `Common patterns`
- repeat important API names and command names in prose near the heading
- use `Agents.md` as a curated entry page instead of duplicating full docs
- keep frontmatter aligned with actual body content; do not use metadata to paper over weak headings
- prefer `aliases` for alternate wording, and reserve `entry_for` for explicit command/API entry points

## Validation

Keep executable retrieval checks in a separate validation document so this file stays normative.

Use [docs/docs-validation.md](docs/docs-validation.md) for:

- search ranking checks
- alias and `entry_for` checks
- `read` / `read-lines` resolver checks
- module-scope regression checks

When changing retrieval logic or frontmatter conventions:

- update this file if the rules changed
- update the validation document if the expected behavior changed
- rerun the validation commands from the validation document

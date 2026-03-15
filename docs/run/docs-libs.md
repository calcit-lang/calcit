# Documentation & Libraries

Calcit includes built-in commands to navigate the language guidebook and discover community libraries.

## Guidebook Access (`docs`)

The `docs` subcommand allows you to read the language guidebook (like this one) without leaving the terminal.

### Reading Chapters

```bash
# List all chapters in the guidebook
cr docs list

# Read a specific file (fuzzy matching supported)
cr docs read run.md

# List headings in a file (best first step before narrowing)
cr docs read run.md

# Jump by heading keyword(s)
cr docs read run.md quick start

# Search for keywords across all chapters
cr docs search "polymorphism"
```

### Advanced Navigation (`read`)

`cr docs read` supports fuzzy heading matching to jump straight to a section:

```bash
# Display the "Quick start" section of run.md
cr docs read run.md "Quick start"

# Exclude subheadings from the output
cr docs read run.md "Quick start" --no-subheadings
```

### Precision Reading (`read-lines`)

Use `read-lines` for large files where you need a specific range:

```bash
# Read 50 lines starting from line 100 of common-patterns.md
cr docs read-lines common-patterns.md --start 100 --lines 50
```

## Fast Navigation Patterns

### Pattern 1: Discover headings first, then narrow

```bash
cr docs read query.md
cr docs read query.md usages
```

### Pattern 2: Search globally, then open exact chapter

```bash
cr docs search trait
cr docs read traits.md
```

## Library Discovery (`libs`)

The `libs` subcommand helps you find and understand Calcit modules.

### Searching Registry

```bash
# Search for libraries related to "web"
cr libs search web
```

### Reading Readmes

You can read the documentation of any official library, even if not installed locally:

```bash
# Show README of 'respo' module
cr libs readme respo

# Read a specific markdown file inside package
cr libs readme respo -f Skills.md
```

### Scanning for Documentation

```bash
# List all markdown files inside the local 'memof' module
cr libs scan-md memof
```

## Collaborative validation (`check-md`)

`docs check-md` is used to verify that code blocks in your markdown documentation are correct and runnable:

```bash
cr docs check-md README.md
```

It supports specific block types:

- `cirru`: Run and validate.
- `cirru.no-run`: Validate syntax and preprocessing without running.
- `cirru.no-check`: Skip checking (illustrative).

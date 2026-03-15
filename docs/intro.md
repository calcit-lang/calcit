# Introduction

Calcit is a scripting language that combines the power of Clojure-like functional programming with modern tooling and hot code swapping.

> An interpreter for Calcit snapshots with hot code swapping support, built with Rust.

Calcit is primarily inspired by ClojureScript and designed for interactive development. It can run natively via the Rust interpreter or compile to JavaScript in ES Modules syntax for web development.

## Key Features

- **Immutable persistent data structures** - All data is immutable by default using ternary tree implementations
- **Structural editing** - Visual tree-based code editing with Calcit Editor
- **Hot code swapping** - Live code updates during development without losing state
- **JavaScript interop** - Seamless integration with JS ecosystem and ES Modules
- **Indentation-based syntax** - Alternative to parentheses for cleaner code
- **Static type analysis** - Compile-time type checking and error detection
- **MCP (Model Context Protocol)** server - Tool integration for AI assistants
- **Fast compilation** - Rust-based interpreter with excellent performance

## Quick Start

You can [try Calcit WASM build online](http://repo.calcit-lang.org/calcit-wasm-play/) for simple snippets, or see the [Quick Reference](quick-reference.md) for common commands and syntax.

Install Calcit via Cargo:

```bash
cargo install calcit
cargo install calcit-bundler  # For indentation syntax
cargo install caps-cli        # For package management
```

## Design Philosophy

Calcit experiments with several interesting ideas:

- **Code as data** - Code is stored in EDN snapshot files (`.cirru`), enabling structural editing and powerful metaprogramming
- **Pattern matching** - Tagged unions and enum types with compile-time validation
- **Type inference** - Static analysis without requiring extensive type annotations
- **Incremental compilation** - Hot reload with `.compact-inc.cirru` for fast iteration
- **Ternary tree collections** - Custom persistent data structures optimized for performance
- **File-as-key/value model** - MCP server integration uses Markdown docs as knowledge base

Most other features are inherited from ClojureScript. Calcit-js is commonly used for web development with [Respo](https://respo-mvc.org/), a virtual DOM library migrated from ClojureScript.

## Use Cases

- **Web development** - Compile to JS and use with Respo or other frameworks
- **Scripting** - Fast native execution for CLI tools and automation
- **Interactive development** - REPL-driven development with hot code swapping
- **Teaching** - Clean syntax and structural editor for learning functional programming

For more details, see [Overview](intro/overview.md) and [From Clojure](intro/from-clojure.md).

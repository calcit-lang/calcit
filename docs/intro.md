---
title: "Introduction"
scope: "core"
kind: "hub"
category: "intro"
aliases:
  - "introduction"
  - "getting started"
  - "language overview"
---
# Introduction

Calcit is a typed functional language designed for interactive programs, real-time web applications, and concise native scripting.

> One language model across a Rust interpreter and JavaScript ES Module output, with state-preserving reload and typed host boundaries.

Calcit runs natively through its Rust interpreter and compiles to JavaScript ES Modules for browser and Node.js applications. Its current language model combines immutable persistent data, code-as-data macros, nominal structs and enums, traits and method dispatch, Option/Result composition, pattern matching, static analysis, and typed host boundaries.

## Key Features

- **Immutable persistent data structures** - All data is immutable by default using ternary tree implementations
- **Nominal types and traits** - Model domain values explicitly and expose capabilities as methods
- **Hot code swapping** - Live code updates during development without losing application state
- **JavaScript interop** - Seamless integration with JS ecosystem and ES Modules
- **Indentation-based syntax** - Alternative to parentheses for cleaner code
- **Static type analysis** - Compile-time checking across functions, enums, structs, traits, and FFI contracts
- **MCP (Model Context Protocol)** server - Tool integration for AI assistants
- **Fast compilation** - Rust-based interpreter with excellent performance

## Quick Start

You can [try Calcit WASM build online](http://repo.calcit-lang.org/calcit-wasm-play/) for simple snippets, or see the [Quick Reference](quick-reference.md) for common commands and syntax.

Install Calcit via Cargo:

```bash
cargo install calcit
```

## Design Philosophy

Calcit experiments with several interesting ideas:

- **Code as data** - Canonical source is stored in `calcit.cirru`, enabling structural CLI edits and metaprogramming
- **Pattern matching** - Tagged unions and enum types with compile-time validation
- **Type inference** - Static analysis without requiring extensive type annotations
- **Deterministic updates** - Keep business-state transitions serial and explicit while asynchronous work remains at system boundaries
- **Ternary tree collections** - Custom persistent data structures optimized for performance
- **Typed capabilities** - Traits and method-oriented APIs keep reusable behavior explicit without erasing domain types

One central Calcit use case is a real-time application built with [Respo](https://respo-mvc.org/), Recollect, WebSocket modules, and [Calcium Workflow](https://github.com/Cumulo/calcium-workflow): the browser sends typed operations, the server applies deterministic state updates, and revisioned diff/patch messages incrementally synchronize client projections. See [Real-time Application Model](intro/realtime-applications.md).

## Use Cases

- **Real-time web applications** - Share typed operations and revisioned incremental projections across browser and server
- **Scripting** - Fast native execution for CLI tools and automation
- **Interactive development** - REPL-driven development with hot code swapping
- **Typed native modules** - Expose system capabilities through C FFI while preserving typed Calcit APIs

For more details, see [Overview](intro/overview.md), [Real-time Application Model](intro/realtime-applications.md), and [Historical Influences and Migration Notes](intro/from-clojure.md).

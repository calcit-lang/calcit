# Features

Calcit inherits most features from Clojure/ClojureScript while adding its own innovations:

## Core Features

- **Immutable persistent data structures** - All data is immutable by default using ternary tree implementations
- **Functional programming** - First-class functions, higher-order functions, closures
- **Lisp syntax** - Code as data, powerful macro system with hygienic macros
- **Hot code swapping** - Live code updates during development without state loss
- **JavaScript interop** - Seamless integration with JS ecosystem via ES Modules
- **Static type analysis** - Compile-time type checking and error detection

## Unique to Calcit

- **Indentation-based syntax** - Alternative to parentheses using `bundle_calcit`, similar to Python/Haskell
- **Structural editing** - Visual tree-based code editing with Calcit Editor (Electron app)
- **ES Modules output** - Modern JavaScript module format, tree-shakeable
- **MCP integration** - Model Context Protocol server for AI assistant tool integration
- **Ternary tree collections** - Custom persistent data structures optimized for Rust
- **Incremental compilation** - Fast hot reload with `.compact-inc.cirru` format
- **Pattern matching** - Tagged unions with compile-time validation
- **Record types** - Lightweight structs with field access validation
- **Traits & method dispatch** - Attach capability-based methods to values, with explicit disambiguation when needed

## Language Features

For detailed information about specific features:

- [List](features/list.md) - Persistent vectors and list operations
- [HashMap](features/hashmap.md) - Key-value data structures and operations
- [Macros](features/macros.md) - Code generation and syntax extension
- [JavaScript Interop](features/js-interop.md) - Calling JS from Calcit and vice versa
- [Imports](features/imports.md) - Module system and dependency management
- [Polymorphism](features/polymorphism.md) - Object-oriented programming patterns
- [Traits](features/traits.md) - Capability-based method dispatch and explicit trait calls
- [Static Analysis](features/static-analysis.md) - Type checking and compile-time validation

## Quick Find by Task

Use this section as a keyword index for `cr docs read`:

- **Collections**: list, map, set, tuple, record
- **Pattern Matching**: enum, tag-match, tuple-match, result
- **Types**: static-analysis, assert-type, optional, variadic
- **Methods**: trait, impl-traits, method dispatch, trait-call
- **Interop**: js interop, async, promise, js-await
- **Architecture**: imports, namespace, module, dependency

Task-oriented jump map:

- Data transforms → [List](features/list.md), [HashMap](features/hashmap.md), [Sets](features/sets.md)
- Domain modeling → [Records](features/records.md), [Enums](features/enums.md), [Tuples](features/tuples.md)
- Type safety → [Static Analysis](features/static-analysis.md), [Error Handling](features/error-handling.md)
- Extensibility → [Macros](features/macros.md), [Traits](features/traits.md), [Polymorphism](features/polymorphism.md)
- Runtime integration → [JavaScript Interop](features/js-interop.md), [Imports](features/imports.md)

## Development Features

- **Type inference** - Automatic type inference from literals and expressions
- **Compile-time checks** - Arity checking, field validation, bounds checking
- **Error handling** - Rich stack traces and error messages with source locations
- **Package management** - Git-based dependency system with `caps` CLI tool
- **Hot module replacement** - Fast iteration with live code updates
- **REPL integration** - Interactive development with `cr eval` mode
- **Bundle mode** - Single-file deployment with `cr bundle`

## Type System

Calcit's static analysis provides:

- **Function arity checking** - Validates argument counts at compile time
- **Record field validation** - Checks field names exist in record types
- **Tuple bounds checking** - Validates tuple index access
- **Enum variant validation** - Ensures correct enum construction
- **Method existence checking** - Verifies methods exist for types
- **Recur arity validation** - Checks recursive calls have correct arguments
- **Return type validation** - Matches function return types with declarations

## Performance

- **Native execution** - Rust interpreter for fast CLI tools and scripting
- **Zero-cost abstractions** - Persistent data structures with minimal overhead
- **Lazy sequences** - Efficient processing of large datasets
- **Optimized compilation** - JavaScript output with tree-shaking support

Calcit is designed to be familiar to Clojure developers while providing modern tooling, type safety, and excellent development experience.

---
title: "Historical Influences and Migration Notes"
scope: "core"
kind: "guide"
category: "intro"
aliases:
  - "clojure migration"
  - "clojurescript migration"
  - "from clojure"
---
# Historical Influences and Migration Notes

Early Calcit borrowed important ideas from Clojure and ClojureScript, including immutable persistent data, namespaces, code-as-data macros, higher-order functions, and interactive development. That history can help readers recognize some surface concepts, but it no longer defines the language.

Modern Calcit has its own semantic center:

- nominal structs and enums with validated construction and pattern matching;
- traits, implementations, and method-oriented capability APIs;
- static type analysis with generics, Option, Result, and explicit Dynamic boundaries;
- canonical Cirru source snapshots and structural CLI editing;
- native Rust execution, typed C FFI capabilities, and JavaScript ES Module output;
- deterministic real-time application updates with revisioned diff/patch synchronization.

Do not assume a Clojure form, argument order, collection delimiter, polymorphism rule, or host interop behavior applies to Calcit. Query Calcit directly:

```bash
calcit query examples calcit.core/map
calcit query type "'String"
calcit docs search trait
```

Useful migration differences include:

| Area | Calcit behavior |
| --- | --- |
| Source syntax | Cirru indentation, `$`, and local parentheses build syntax trees; `[]`, `{}`, and `#{}` are constructor symbols |
| Collection transforms | Collection arguments generally come before callbacks, such as `map xs f` |
| Domain values | Prefer nominal `defstruct` and `defenum` definitions over untyped map/tag conventions |
| Polymorphism | Prefer traits and methods, with explicit trait calls for disambiguation |
| Recoverable absence/failure | Use Option and Result methods and matching rather than nil/exception conventions |
| Native extension | Typed C FFI modules expose capabilities through Calcit-facing APIs |
| Web applications | JavaScript ES Modules, Respo projections, and revisioned WebSocket synchronization are first-class ecosystem patterns |

This page is intentionally a migration aid, not the primary explanation of Calcit. New documentation should introduce Calcit concepts on their own terms and use comparisons only when they prevent a concrete migration mistake.

# Data Types

Calcit provides several core data types, all immutable by default for functional programming:

## Primitive Types

- **Bool**: `true`, `false`
- **Number**: `f64` in Rust, Number in JavaScript (`1`, `3.14`, `-42`)
- **Tag**: Immutable strings starting with `:` (`:keyword`, `:demo`) - similar to Clojure keywords
- **String**: Text data with special prefix syntax (`|text`, `"|with spaces"`)

## Collection Types

- **Vector**: Ordered collection serving both List and Vector roles (`[] 1 2 3`)
- **HashMap**: Key-value pairs (`{} (:a 1) (:b 2)`)
- **HashSet**: Unordered unique elements (`#{} :a :b :c`)

## Function Types

- **Function**: User-defined functions and built-in procedures
- **Proc**: Internal procedure type for built-in functions

## Implementation Details

- **Rust runtime**: Uses [rpds](https://github.com/orium/rpds) for HashMap/HashSet and [ternary-tree](https://github.com/calcit-lang/ternary-tree.rs/) for vectors
- **JavaScript runtime**: Uses [ternary-tree.ts](https://github.com/calcit-lang/ternary-tree.ts) for all collections

All data structures are persistent and immutable, following functional programming principles. For detailed information about specific types, see:

- [String](data/string.md) - String syntax and Tags
- [Persistent Data](data/persistent-data.md) - Implementation details
- [EDN](data/edn.md) - Data notation format

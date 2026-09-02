---
title: "Quick Reference"
scope: "core"
kind: "reference"
category: "reference"
aliases:
  - "cheatsheet"
  - "cheat sheet"
  - "quick commands"
  - "quick reference"
entry_for:
  - "calcit --version"
  - "cargo run --bin calcit -- -v"
---

# Quick Reference

This page provides a quick overview of key Calcit concepts and commands for rapid lookup.

## Installation & Setup

```bash
# Install Rust first
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install Calcit
cargo install calcit

# Test installation
calcit eval "echo |done"
```

## Core Commands

- `calcit` - Run Calcit program from `calcit.cirru`; retired `compact.cirru` inputs receive migration guidance
- `calcit eval "code"` - Evaluate code snippet
- `calcit js` - Generate JavaScript
- `calcit query ...` - Query definitions/usages/search
- `calcit docs ...` - Navigate calcit docs and installed module docs with `scopes -> list -> sections -> read`
- `calcit docs remote-libs ...` - Search package registry and read package README files
- `cr-mcp` - Start MCP server for tool integration

### CLI Options

- `--watch` / `-w` - Watch files and rerun/rebuild on changes
- `--disable-stack` - Disable stack trace for errors
- `--skip-arity-check` - Skip arity check in JS codegen
- `--emit-path <path>` - Specify output path for JS (default: `js-out/`)
- `--init-fn <fn>` - Specify main function
- `--reload-fn <fn>` - Specify reload function for hot reloading
- `--entry <entry>` - Use config entry
- `--reload-libs` - Force reload libs data during hot reload
- `--watch-dir <path>` - Watch assets changes

### Markdown Checking

- See [CLI Options](./run/cli-options.md#markdown-code-checking) for `check-md` usage and mode guidance.

### Docs Navigation (Fast)

- `calcit docs list` - list available chapters
- `calcit docs list --module <name>` - list docs from one installed module
- `calcit docs scopes` - list available doc scopes (`calcit` and installed modules)
- `calcit docs sections <file>` - list headings in one chapter
- `calcit docs sections <file> --module <name>` - list headings in one module doc
- `calcit docs read <file>` - read a full calcit doc
- `calcit docs read <file> <keyword...>` - fuzzy jump by heading keywords
- `calcit docs read <file> --module <name>` - read a full module doc
- `calcit docs read-lines <file> --start <start> --lines <lines>` - precise line-range reading
- `calcit docs search <keyword>` - global keyword search
- `calcit docs search <keyword> --module <name>` - search installed module docs directly
- `calcit docs remote-libs search <keyword>` - search remote library registry
- `calcit docs remote-libs readme <package>` - read a remote or installed package README
- `calcit docs graph build` - build the structured documentation relationship cache
- `calcit docs graph path <from> <to>` - find a short path between knowledge nodes
- `calcit docs graph explain <namespace/definition> --full` - show definition details and linked docs

## Data Types

- **Numbers**: `1`, `3.14`
- **Strings**: `|text`, `"|with spaces"`, `"\"escaped"`(old style)
- **Tags**: `:keyword` (interned immutable identifiers for fields, variants, and protocol labels)
- **Lists**: `[] 1 2 3`
- **HashMaps**: `{} (:a 1) (:b 2)`
- **HashSets**: `#{} :a :b :c`
- **Anonymous Enums**: `%:: _ :tag 1 2` (or `:: :tag 1 2`) - short-lived tagged values
- **Anonymous Structs**: `%{} _ (:key1 val1) (:key2 val2)` - short-lived fixed-field values
- **Structs**: `defstruct Point (:x 'Number) (:y 'Number)` and `%{} Point ...`
- **Enums**: `defenum Result (:ok ..) (:err 'String)` and `%:: Result ...`
- **Refs/Atoms**: `atom 0` - mutable references
- **Buffers**: `&buffer 0x01 0x02` - binary data

## Basic Syntax

```cirru
; Function definition

defn add (a b) (+ a b)
```

```cirru
; Conditional

let
    x 1
  if (> x 0) |positive |negative
```

```cirru
; Let binding

let
    a 1
    b 2
  + a b
```

```cirru
; Thread macro

-> (range 10)
  filter $ fn (x) (> x 5)
  map inc
```

## Type Annotations

```cirru
let
    ; Local function with type annotations
    add $ fn (a b)
      hint-fn $ {}
        :args $ [] 'Number 'Number
        :return 'Number
      + a b
    ; Local variadic function
    sum $ fn (& xs)
      hint-fn $ {} (:rest 'Number) (:return 'Number)
      apply + xs
    ; Struct definition
    User $ defstruct User (:name 'String) (:age 'Number) (:email 'String)
    x 42
  ; Type assertion $ composable check, returns original value
  assert-type x 'Number
  [] (add 3 4) (sum 1 2 3) x
```

Namespace-level definitions use `:schema`, for example:

```cirru
defn add (a b) (+ a b)
```

`schema` can be attached separately:

```cirru
:: 'Fn $ {}
  :args $ [] 'Number 'Number
  :return 'Number
```

### Built-in Types

- `'Number`, `'String`, `'Bool`, `'Nil`, `'Dynamic`
- `'List`, `'Map`, `'Set`, `'Struct`, `'Enum`, `'StructDef`, `'EnumDef`, `'Fn`
- `'Dynamic` - wildcard type (default when no annotation)
- Generic types (Cirru style):

```cirru
let
    t1 $ :: 'List 'Number
    t2 $ :: 'Map 'String
    t3 $ :: 'Fn
      {}
        :args $ [] 'Number
        :return 'String
  [] t1 t2 t3
```

### Static Checks (Compile-time)

- **Arity checking**: Function call argument count validation
- **Struct field checking**: Validates required field names in struct access
- **Enum index bounds**: Ensures enum payload indices are valid
- **Enum tag matching**: Validates tags in `&case` and `&extract-case`
- **Method validation**: Checks method names and class types
- **Recur arity**: Validates recur argument count matches function params

### Method & Access Syntax

- Method call: `xs .map inc` when the receiver type is known; prefix `.map xs inc` remains compatible for dynamic boundaries
- Required named-Struct field access: use `(:name value)` or receiver-first `value.:name`; the checker validates the declared type and lowers it to indexed access
- Optional Map lookup: use `get` and handle its `Option`; do not use `&struct:get` as application syntax
- An unresolved short nominal receiver such as `'Router` means its declaration context was lost; qualify the schema (for example `'app.schema/Router`) instead of hiding the diagnostic with `&struct:get`
- Trait/impl declarations prefer dot method keys like `.foo`; legacy tag keys like `:foo` remain compatible but emit a default warning in `deftrait`/`defimpl`

## File Structure

- `calcit.cirru` - Preferred runtime snapshot and structural-editing source
- `compact.cirru` - Retired runtime snapshot filename; migrate it to `calcit.cirru` with Calcit 0.13.48 as the final compatibility release
- `deps.cirru` - Dependencies
- `.compact-inc.cirru` - Hot reload trigger, including incremental changes

## Common Functions

### Math

- `+`, `-`, `*`, `/` - arithmetic (variadic)
- `&+`, `&-`, `&*`, `&/` - binary arithmetic
- `inc`, `dec` - increment/decrement
- `pow`, `sqrt`, `round`, `floor`, `ceil`
- `sin`, `cos` - trigonometric functions
- `&max`, `&min` - binary min/max
- `&number:fract` - fractional part
- `&number:rem` - remainder
- `&number:format` - format number
- `bit-shl`, `bit-shr`, `bit-and`, `bit-or`, `bit-xor`, `bit-not`

### List Operations

- `[]` - create list
- `append`, `prepend` - add elements
- `concat` - concatenate lists
- `nth`, `first`, `rest`, `last` - access elements
- `count`, `empty?` - list properties
- `slice` - extract sublist
- `reverse` - reverse list
- `sort`, `sort-by` - sorting
- `map`, `filter`, `reduce` - functional operations
- `foldl`, `foldl-shortcut`, `foldr-shortcut` - folding
- `range` - generate number range
- `take`, `drop` - slice operations
- `distinct` - remove duplicates
- `&list:contains?`, `&list:includes?` - membership tests

### Map Operations

- `{}` or `&{}` - create map
- `&map:get` - get value by key
- `&map:assoc`, `&map:dissoc` - add/remove entries
- `&map:merge` - merge maps
- `&map:contains?`, `&map:includes?` - key membership
- `keys`, `vals` - extract keys/values
- `to-pairs`, `pairs-map` - convert to/from pairs
- `&map:filter`, `&map:filter-kv` - filter entries
- `&map:common-keys`, `&map:diff-keys` - key operations

### Set Operations

- `#{}` - create set
- `include`, `exclude` - add/remove elements
- `union`, `difference`, `intersection` - set operations
- `&set:includes?` - membership test
- `&set:to-list` - convert to list

### String Operations

- `str` - concatenate to string
- `str-spaced` - join with spaces
- `&str:concat` - binary concatenation
- `trim`, `split`, `split-lines` - string manipulation
- `starts-with?`, `ends-with?` - prefix/suffix tests
- `&str:slice` - extract substring
- `&str:replace` - replace substring
- `str-find-index`, string `.find-index` - find position as `Option<Number>`
- `&str:find-index` - internal raw search primitive (`-1` when absent)
- `&str:contains?`, `&str:includes?` - substring tests
- `&str:pad-left`, `&str:pad-right` - padding
- `parse-float` - parse number from string
- `get-char-code`, `char-from-code` - character operations
- `&str:escape` - escape string

### Enum Operations

- `defenum` - define a named enum
- `%::` - create a named enum value, or an anonymous value with `%:: _ ...`
- `::` - shorthand for an anonymous enum value
- `&enum:nth` - access the variant tag or payload by index
- `&enum:assoc` - update a payload position
- `&enum:count` - count the variant tag and payload positions
- `&enum:params` - get payload parameters
- `enum-definition` - get the definition as `Option<EnumDef>`
- `destruct-list`, `destruct-map`, `destruct-set`, `destruct-str` - split collections using nominal `*Destruct` enums
- `enum?`, `enum-def?` - distinguish values from definitions

### Struct Operations

- `defstruct` - define a struct type with typed fields
- `%{}` - create a named struct value, or an anonymous value with `%{} _ ...`
- `%{}?` - legacy partial Struct constructor (unset fields default to nil;
  rejected by `--strict-types` with `E_PARTIAL_STRUCT_NIL_FILL`)
- `&%{}` - low-level struct constructor (flat key-value pairs, no type check)
- `struct-with` - update multiple declared fields
- `&struct:get` - internal/dynamic-boundary field lookup; normal typed code must use `(:field value)`
- `&struct:assoc` - set a declared field (low-level)
- `struct-definition` - get the definition as `Option<StructDef>`
- `&struct:matches?` - type check
- `&struct:from-map` - convert from map
- `&struct:to-map` - convert to map
- `&struct:get-name` - get the tag name of the struct definition
- `struct?`, `struct-def?` - distinguish values from definitions

### Struct & Enum Operations

- `defstruct` - define struct type
- `defenum` - define enum type
- `&struct-def:new`, `&enum-def:new` - internal definition constructors
- `struct?`, `enum?` - value predicates
- `struct-def?`, `enum-def?` - definition predicates
- `&enum-def:has-variant?` - check a declared variant
- `&enum-def:variant-arity` - get declared variant arity
- `match` - native pattern matching on named enums
- `tag-match` - fallback matching for anonymous enums

## Traits & Methods

- `deftrait` - define a trait (method set + type signatures)
- `impl-origin` - get an impl's trait origin as `Option<Trait>`
- `defimpl` - define a nominal impl value for a trait: `defimpl ImplName Trait ...`
- `impl-traits` - attach impl records to a struct/enum definition (user impls: later impls override earlier ones for same method name)
- `.method` - normal method dispatch
- `&trait-call` - explicit trait method call: `&trait-call Trait :method receiver & args`
- `&methods-of` - list runtime-available methods (strings including leading dot)
- `&inspect-methods` - print impl/method resolution to stderr, returns the value unchanged
- `assert-traits` - runtime check that a value implements a trait, returns the value unchanged

### Ref/Atom Operations

- `atom` - create atom
- `&atom:deref` or `deref` - read value
- `reset!` - set value
- `swap!` - update with function
- `add-watch`, `remove-watch` - observe changes
- `ref?` - predicate

### Type Predicates

- `nil?`, `some?` - nil checks
- `number?`, `string?`, `tag?`, `symbol?`
- `list?`, `map?`, `set?`, `struct?`, `enum?`
- `struct-def?`, `enum-def?`, `ref?`
- `fn?`, `macro?`

### Control Flow

- `if` - conditional
- `when`, `when-not` - single-branch conditionals
- `cond` - multi-way conditional; requires a final `(true value)` branch
- `case` - pattern matching on values; raises when no pattern matches
- `&case` - internal case macro
- `match` - preferred named-enum pattern matching
- `tag-match` - fallback anonymous-enum pattern matching
- `struct-match` - struct pattern matching
- `list-match` - list destructuring match
- `if-let` - bind an `Option<T>` payload with explicit some/none branches
- `when-let` - run a body for `%some` and return `Option<R>`

Nested updates are nominal as well: `update-in` passes `Option<T>` to its
updater, and `dissoc-in` treats an empty path as a no-op. On a fully typed nested
Map, non-empty literal paths in `get-in`, `assoc-in`, and `update-in` compile to
direct typed lookup/association chains with single evaluation of each argument.
Dynamic paths, open containers, and mixed traversal deliberately keep the
runtime compatibility API. Public lookup and
positional APIs (`get`, `get-in`, `first`, `last`, and collection `nth`) return
`Option<T>`. Accessing a statically known struct field with `(:field value)` or
receiver-first `value.:field` returns the field's declared type directly and is
lowered to indexed access; an undeclared field is a diagnostic, not `nil`.
`get` does not read Struct fields, and Struct has no public positional `.nth`.

### Threading Macros

- `->` - thread first
- `->>` - thread last
- `->%` - thread with `%` placeholder

### Other Macros

- `let` - local bindings
- `defn` - define function
- `defmacro` - define macro
- `fn` - anonymous function
- `quote`, `quasiquote` - code as data
- `macroexpand`, `macroexpand-all` - debug macros
- `assert`, `assert=` - assertions
- `&doseq` - side-effect iteration
- `for` - list comprehension

### Meta Operations

- `type-of` - get type tag
- `turn-string`, `turn-symbol`, `turn-tag` - type conversion
- `identical?` - reference equality
- `recur` - tail recursion
- `generate-id!` - unique ID generation
- `cpu-time` - timing
- `&get-os`, `&get-calcit-backend` - environment info

### EDN/Data Operations

- `parse-cirru-edn`, `format-cirru-edn` - EDN serialization
- `parse-cirru`, `format-cirru` - Cirru syntax
- `&data-to-code` - convert data to code
- `pr-str` - print to string

### Effects/IO

- `echo`, `println` - output
- `fs:path` - construct a nominal `FsPath` from a UTF-8 String without normalization
- `FsPath .read-text`, `.read-dir`, `.walk-dir`, `.write-text` - recoverable file effects returning `Result` (native/JS; unavailable in WASM)
- `try-read-file`, `try-read-dir`, `try-write-file` - String-path compatibility functions returning `Result`
- `read-file`, `read-dir`, `write-file` - raising compatibility primitives (native/JS; unavailable in WASM)
- `ffi:task`, `FfiTask .cancel` / `.cancel-with` - nominal native async task lifecycle API
- `ffi:response`, `FfiResponse .resolve` / `.reject` - nominal exactly-once native response API
- `get-env` - environment variables
- `raise` - throw error
- `quit!` - exit program

For detailed information, see the specific documentation files in the table of contents.

# Quick Reference

This page provides a quick overview of key Calcit concepts and commands for rapid lookup.

## Installation & Setup

```bash
# Install Rust first
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install Calcit
cargo install calcit

# Test installation
cr eval "echo |done"
```

## Core Commands

- `cr` - Run Calcit program (default: `compact.cirru`)
- `cr eval "code"` - Evaluate code snippet
- `cr js` - Generate JavaScript
- `cr ir` - Generate IR representation
- `cr query ...` - Query definitions/usages/search
- `cr docs ...` - Read/search guidebook docs
- `cr libs ...` - Search/read library docs
- `cr-mcp` - Start MCP server for tool integration

### CLI Options

- `--watch` / `-w` - Watch files and rerun/rebuild on changes
- `--once` / `-1` - Run once (compatibility flag; default is already once)
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

- `cr docs list` - list available chapters
- `cr docs read <file>` - list headings in one chapter
- `cr docs read <file> <keyword...>` - fuzzy jump by heading keywords
- `cr docs read-lines <file> -s <start> -n <lines>` - precise line-range reading
- `cr docs search <keyword>` - global keyword search

## Data Types

- **Numbers**: `1`, `3.14`
- **Strings**: `|text`, `"|with spaces"`, `"\"escaped"`
- **Tags**: `:keyword` (immutable strings, like Clojure keywords)
- **Lists**: `[] 1 2 3`
- **HashMaps**: `{} (:a 1) (:b 2)`
- **HashSets**: `#{} :a :b :c`
- **Tuples**: `:: :tag 1 2` - tagged unions with class support
- **Records**: `%{} RecordName (:key1 val1) (:key2 val2)`, similar to structs
- **Structs**: `defstruct Point (:x :number) (:y :number)` - record type definitions
- **Enums**: `defenum Result (:ok ..) (:err :string)` - sum types
- **Refs/Atoms**: `atom 0` - mutable references
- **Buffers**: `&buffer 0x01 0x02` - binary data

## Basic Syntax

```cirru
; Function definition
defn add (a b)
  + a b
```

```cirru
; Conditional
let ((x 1))
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
      hint-fn $ {} (:args ([] :number :number)) (:return :number)
      + a b
    ; Local variadic function
    sum $ fn (& xs)
      hint-fn $ {} (:rest :number) (:return :number)
      apply + xs
    ; Struct definition
    User $ defstruct User (:name :string) (:age :number) (:email :string)
    x 42
  ; Type assertion (composable check, returns original value)
  assert-type x :number
  [] (add 3 4) (sum 1 2 3) x
```

Namespace-level definitions use `:schema`, for example:

```cirru
defn add (a b)
  + a b
```

`schema` can be attached separately:

```cirru
:: :fn $ {} (:args $ [] :number :number) (:return :number)
```

### Built-in Types

- `:number`, `:string`, `:bool`, `:nil`, `:dynamic`
- `:list`, `:map`, `:set`, `:record`, `:fn`, `:tuple`
- `:dynamic` - wildcard type (default when no annotation)
- Generic types (Cirru style):

```cirru
let
    t1 $ :: :list :number
    t2 $ :: :map :string
    t3 $ :: :fn $ {}
      :args $ [] :number
      :return :string
  [] t1 t2 t3
```

### Static Checks (Compile-time)

- **Arity checking**: Function call argument count validation
- **Record field checking**: Validates field names in record access
- **Tuple index bounds**: Ensures tuple indices are valid
- **Enum tag matching**: Validates tags in `&case` and `&extract-case`
- **Method validation**: Checks method names and class types
- **Recur arity**: Validates recur argument count matches function params

### Method & Access Syntax

- Method call: `.map xs inc` (or shorthand `xs.map inc`)
- Tag access (map key): prefer `obj.:name` over legacy `(:name obj)`
- Trait/impl declarations prefer dot method keys like `.foo`; legacy tag keys like `:foo` remain compatible but emit a default warning in `deftrait`/`defimpl`

## File Structure

- `calcit.cirru` - Editor snapshot (source for structural editing)
- `compact.cirru` - Runtime format (compiled, `cr` command actually uses this)
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
- `&str:find-index` - find position
- `&str:contains?`, `&str:includes?` - substring tests
- `&str:pad-left`, `&str:pad-right` - padding
- `parse-float` - parse number from string
- `get-char-code`, `char-from-code` - character operations
- `&str:escape` - escape string

### Tuple Operations

- `::` - create tuple (shorthand)
- `%::` - create tuple with class
- `&tuple:nth` - access element by index
- `&tuple:assoc` - update element
- `&tuple:count` - get element count
- `&tuple:class` - get class
- `&tuple:params` - get parameters
- `&tuple:enum` - get enum tag
- `&tuple:with-class` - change class

### Record Operations

- `defstruct` - define a struct type with typed fields
- `%{}` - create a record instance from a struct
- `%{}?` - create a partial record (unset fields default to nil)
- `&%{}` - low-level record constructor (flat key-value pairs, no type check)
- `record-with` - update multiple fields, returns new record
- `&record:get` - get field value
- `&record:assoc` - set field value (low-level)
- `&record:struct` - get the struct definition the record was created from
- `&record:matches?` - type check
- `&record:from-map` - convert from map
- `&record:to-map` - convert to map
- `&record:get-name` - get tag name of the record's struct
- `record?`, `struct?` - predicates

### Struct & Enum Operations

- `defstruct` - define struct type
- `defenum` - define enum type
- `&struct::new`, `&enum::new` - create instances
- `struct?`, `enum?` - predicates
- `&tuple:enum-has-variant?` - check variant
- `&tuple:enum-variant-arity` - get variant arity
- `tag-match` - pattern matching on enums

## Traits & Methods

- `deftrait` - define a trait (method set + type signatures)
- `defimpl` - define an impl record for a trait: `defimpl ImplName Trait ...`
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
- `list?`, `map?`, `set?`, `tuple?`
- `record?`, `struct?`, `enum?`, `ref?`
- `fn?`, `macro?`

### Control Flow

- `if` - conditional
- `when`, `when-not` - single-branch conditionals
- `cond` - multi-way conditional
- `case` - pattern matching on values
- `&case` - internal case macro
- `tag-match` - enum/tuple pattern matching
- `record-match` - record pattern matching
- `list-match` - list destructuring match
- `field-match` - map field matching

### Threading Macros

- `->` - thread first
- `->>` - thread last
- `->%` - thread with `%` placeholder
- `%<-` - reverse thread

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
- `read-file`, `write-file` - file operations
- `get-env` - environment variables
- `raise` - throw error
- `quit!` - exit program

For detailed information, see the specific documentation files in the table of contents.

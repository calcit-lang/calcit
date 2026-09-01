---
title: "Cirru Extensible Data Notation"
scope: "core"
kind: "reference"
category: "data"
aliases:
  - "cirru edn"
  - "edn notation"
  - "data notation"
entry_for:
  - "calcit cirru parse-edn"
---
# Cirru Extensible Data Notation

Cirru EDN is Calcit's typed data interchange format, inspired by [Clojure EDN](https://github.com/edn-format/edn). Use it when Calcit-specific identity matters; use JSON only when interoperating with JSON systems.

The runtime APIs are:

- `parse-cirru-edn text [type-options]`
- `parse-cirru-edn-as text TypeExpr`
- `decode-map-as value TypeExpr`
- `format-cirru-edn value`

`parse-cirru-edn` is the dynamic API: its result is `Dynamic`, and its optional type map only restores nominal identity. Use `parse-cirru-edn-as` when persisted or external data must enter typed application code.

## Recoverable parsing with Result

User-facing String methods return `Result` instead of raising on malformed input:

```cirru
let
    parsed $ |[] .parse-cirru-edn
    json $ |1 .parse-json
  assert= true $ parsed .ok?
  assert= true $ json .ok?
```

The available methods are `.parse-cirru`, `.parse-cirru-list`, `.parse-cirru-edn`, and `.parse-json`. Their function-form counterparts are named `try-parse-cirru`, `try-parse-cirru-list`, `try-parse-cirru-edn`, and `try-parse-json`. Parse failures carry the parser message in `Result`'s `:err` payload, including Cirru source position and nearby input where available.

Cirru syntax has a closed result type, while Cirru EDN and JSON remain `Result<Dynamic,String>` because their data shapes are open. Use `parse-cirru-edn-as` or `decode-map-as` after the boundary when application code needs a closed nominal type. The original parser procedures remain available for compatibility and still raise on malformed input.

These Result-returning parser methods currently run on the native and JavaScript backends. The WASM backend does not yet support the underlying parser procedures or `try`-based wrappers; the standard WASM validation suite confirms that unsupported parser definitions are skipped without affecting supported exports.

## Strict typed decoding

`parse-cirru-edn-as` is a language syntax that derives a closed decoder graph at compile time, then validates and constructs the complete value recursively:

```cirru.no-run
def Person $ defstruct Person (:name 'String) (:age 'Number)

defn decode-person (raw)
  parse-cirru-edn-as raw Person
```

Container and generic targets use the existing type-expression syntax:

```cirru.no-check
parse-cirru-edn-as "|[] 1 2 3" $ :: 'List 'Number
parse-cirru-edn-as "|%{} :Box (:value |hi)" $ :: Box 'String
```

Successful decoding guarantees the recursive container elements, struct fields, enum variant and payloads, generic arguments, and nominal struct/enum identity all match the target type. It does not coerce strings to numbers, maps to structs, or ordinary lists to enums.

Strict decoder derivation rejects `Dynamic`, bare containers with implicit Dynamic elements, missing generic arguments, unbound type variables/type slots, functions, traits, impls, `JsObject`, and unknown custom types. This is a compile-time error rather than a warning or runtime Dynamic fallback.

Runtime failures include a structural path:

```text
parse-cirru-edn-as failed at $.friends[2].age: expected number, got string
```

The compatibility form raises the failure like `parse-cirru-edn`. New code that expects malformed external input should use the Result-returning syntax:

```cirru.no-run
def Person $ defstruct Person (:name 'String) (:age 'Number)

defn decode-people (raw)
  try-parse-cirru-edn-as raw $ :: 'List Person
```

Its inferred return type is `Result<List<Person>,String>`, not `Result<Dynamic,String>`. Runtime parse and recursive shape failures become `:err` with the same structural context. An invalid `TypeExpr` remains a compile-time error because it is a program definition problem rather than recoverable input.

## Decoding runtime maps into Structs

`decode-map-as` is the companion for data that has already crossed a host boundary and is now an evaluated Calcit value, such as a JSON object returned by a JavaScript FFI. It derives the target shape at compile time and returns the declared type, so it is the typed replacement for ad-hoc map readers and runtime schema libraries. Struct targets consume maps, while list, map, enum, ref, and scalar targets are decoded recursively as well.

Decode exactly once, at the boundary. Passing an already nominal Struct to
`decode-map-as` or `try-decode-map-as` is a compile-time
`E_DECODE_MAP_AS_ALREADY_STRUCT` error; use that Struct directly. This prevents
an updater or history loader from accidentally re-decoding typed state and
failing later with “expected map, got struct”.

It converts a Map to a nominal Struct recursively, rejects unknown keys and missing required fields, and reports the path of a bad nested value. A missing `Option<T>` field becomes `%none`; a present raw `T` becomes `%some T`. Already-wrapped `%some` and `%none` values are also accepted.

```cirru.no-run
defstruct Response (:code 'Number)
  :message $ :: 'Option 'String
  :body 'Dynamic

; `raw` may be { :code 200, :message |ok, :body ... }
; the result is Response(:code 200, :message (%some |ok), :body ...)
; omitting :message produces (%none)
```

Unlike the closed text decoder, `decode-map-as` permits an explicitly declared `Dynamic` leaf for an open payload such as an HTTP response body. Keep it at the boundary and decode it again into a closed Struct/Enum before application logic depends on it. It never treats `nil` as an empty map or silently supplies required fields. Native and JavaScript support this syntax; the WASM backend does not currently support typed decoder syntaxes.

Use `try-decode-map-as value TypeExpr` when a host value may legitimately fail validation. It returns `Result<T,String>` and preserves paths such as `$.friends[2].age` in the error payload. `decode-map-as` remains available for compatibility and for boundaries where invalid input should abort immediately.

The raising and Result-returning typed syntaxes are available for native execution and JavaScript code generation. The current WASM backend does not yet support typed EDN decoding.

Map and Set iteration order is not a semantic guarantee. The formatter applies a stable order for readable, reproducible output; callers must not use that order as application data.

## Choosing Cirru EDN or JSON

| Calcit value | Cirru EDN | JSON |
| --- | --- | --- |
| Nil, Bool, Number, String, List | Preserved | Preserved |
| Tag, Symbol | Preserved | Encoded as a JSON string; identity is lost |
| Map | Arbitrary EDN keys preserved | Keys must be Tag or String; parsed object keys become Tag |
| Set | Preserved | Encoded as an array; Set identity is lost |
| Buffer | Preserved | Encoded as a `0x...` string; Buffer identity is lost |
| Enum value | Variant, payloads, and enum name preserved | Encoded as an array; enum identity is lost |
| Struct value | Struct name and fields preserved | Encoded as an object; struct identity is lost |
| Cirru quote | Preserved | Encoded as nested strings and arrays |
| Ref | Encoded as `atom`; parsing creates a new Ref | Unsupported |
| AnyRef, function, trait/impl, mutable builder | Not portable; do not use as interchange data | Unsupported |

`json-stringify` rejects unsupported values and Maps with non-Tag/non-String keys instead of silently inventing a representation. It also rejects non-finite numbers. `json-parse` maps JSON arrays to List and JSON objects to Maps whose keys are Tags.

## Restoring declared struct and enum identity

Cirru EDN always retains the printed struct or enum name. To reconnect parsed values to the exact `defstruct` or `defenum` object (including attached traits), pass an options Map keyed by that name:

```cirru
let
    Person $ defstruct Person (:name 'String)
    encoded $ format-cirru-edn $ %{} Person (:name |Ada)
  parse-cirru-edn encoded $ {}
    :Person $ %{} Person (:name |)
```

Without this options Map, parsing still produces a structurally equivalent anonymous Struct or Enum, but it cannot recover declaration-attached trait implementations from text alone. The options Map does not validate field value types, container elements, enum payloads, generics, or constraints. Prefer `parse-cirru-edn-as` when those guarantees are required.

## Literals

For literals, if written in text syntax, we need to add `do` to make sure it's a line:

```cirru
do nil
```

for a number:

```cirru
do 1
```

for a symbol:

```cirru
do 's
```

Tags use a leading colon:

```cirru
do :k
```

## String escaping

for a string:

```cirru
do |demo
```

or wrap with double quotes to support special characters like spaces:

```cirru
do "|demo string"
```

`\n` `\t` `\"` `\\` are supported.

## Data structures

for a list:

```cirru
[] 1 2 3
```

or nested list inside list:

```cirru
[] 1 2
  [] 3 4
```

HashSet for unordered elements:

```cirru
#{} :a :b :c
```

HashMap:

```cirru
{}
  :a 1
  :b 2
```

also can be nested:

```cirru
{}
  :a 1
  :c $ {}
    :d 3
```

Structs retain their type name and fields:

```cirru
let
    A $ defstruct A (:a 'Number)
  ; Then create an instance in Calcit
  %{} A
    :a 1
```

Named enums use `%:: EnumDef ...`; anonymous enum values use `%:: _ ...` or the `::` shorthand:

```cirru.no-run
let
    SampleResult $ defenum SampleResult (:ok 'Number) (:err 'String)
  %:: SampleResult :ok 1
:: :point 10 20
```

## Quotes

Quoted Cirru is preserved as syntax data. This is used by runtime snapshots (`calcit.cirru`):

```cirru
quote $ def a 1
```

at runtime, its syntax tree uses anonymous enum values:

```cirru
:: 'quote $ [] |def |a |1
```

which means you can eval:

```bash
$ calcit eval "println $ format-cirru-edn $ :: 'quote $ [] |def |a |1"

quote $ def a 1

took 0.027ms: nil
```

and also:

```bash
$ calcit eval 'parse-cirru-edn "|quote $ def a 1"'
took 0.011ms: (:: 'quote ([] |def |a |1))
```

The runtime display uses an anonymous enum shape, but Cirru EDN gives `quote` dedicated parsing and formatting semantics.

## Buffers

Buffers can be created using the `&buffer` function with hex values:

```cirru
&buffer 0x03 0x55 0x77 0xff 0x00
```

## Comments

Comment expressions start with `;`. They occupy nodes in the Cirru syntax tree and are ignored while EDN data is decoded.

Some usages:

```cirru
[] 1 2 3 (; comment) 4 (; comment)
```

```cirru
{}
  ; comment
  :a 1
```

Comments must still obey Cirru indentation because they are syntax-tree nodes, not lexer comments.

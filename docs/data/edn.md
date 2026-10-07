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

## 从文件或标准输入解析

短小数据可以继续直接作为参数传给 `calcit cirru parse-edn`。完整 Snapshot 或其他大型 Cirru EDN 不应塞进命令行参数；Linux、Windows 等平台对单个参数有不同的长度上限。使用 `--file` 可以绕开该限制：

```bash
calcit cirru parse-edn --file calcit.cirru
```

管道调用通过 `--file -` 显式读取标准输入，避免省略参数时意外挂起：

```bash
cat calcit.cirru | calcit cirru parse-edn --file -
```

内联 EDN 与 `--file` 互斥；命令仍只向 stdout 写出一个 JSON 文档，供只接受 JSON 的工具链消费。Calcit 自有数据与工作流继续以 Cirru EDN 为默认格式。

应用代码优先使用 String 的 `.parse-cirru-edn` 方法，得到 `Result<Dynamic,String>`；需要具名函数时，等价的 checked 入口是 `try-parse-cirru-edn text`。原有 `parse-cirru-edn text [type-options]` 会在格式错误时抛错，其可选类型映射只恢复名义身份，并不验证字段与容器元素。

闭合类型数据使用 `try-parse-cirru-edn-as text TypeExpr` 获取 `Result<T,String>`；`parse-cirru-edn-as` 是会抛错的兼容形式。已求值的开放值可用 `decode-map-as value TypeExpr` 验证。序列化入口是 `format-cirru-edn value`。

native 和 JavaScript 的 `format-cirru-edn value inline?` 接受可选 Bool 参数，省略时为 `true`；传 `false` 请求展开式容器布局，解析后的数据不变。标量根节点在两种布局下都使用 `do <值>`，Number 也可独立往返。

WASM 的容器格式化目前只支持省略参数或显式字面量 `true`；`false` 和运行时模式会明确报告 `E_WASM_EDN_FORMAT_MODE`，不静默忽略。已支持的标量布局不受此参数影响，仍保持参数的求值顺序；具体闭合数据和数值支持范围见 [WASM 格式化边界](../../scripts/wasm-validation.md#cirru-edn-格式化边界)。

## Recoverable parsing with Result

严格类型代码优先使用返回 `Result` 的 String 方法，以便处理格式错误：

```cirru
let
    parsed $ |[] .parse-cirru-edn
    json $ |1 .parse-json
  assert= true $ parsed .ok?
  assert= true $ json .ok?
```

`.parse-cirru`、`.parse-cirru-list`、`.parse-cirru-edn` 和 `.parse-json` 都是推荐的 checked 接收者方法；`try-parse-*` 是可按名称查询或作为函数值使用的具名形式，并非另一套首选语义。解析失败的信息位于 `Result` 的 `:err` 分支。旧的 `parse-*`/`json-parse` 抛错入口与 checked 形式不等价，不能直接自动改写调用。

`.parse-cirru` 的成功值是闭合的 `CirruQuote`；`.parse-cirru-list` 的成功值是递归的 String/List 语法数据，目前以 `List<Dynamic>` 表达，不是 `CirruQuote`。两者都要求 String 输入，旧的 `parse-cirru-list` 底层调用也会在严格检查阶段拒绝 Number 等非 String。Cirru EDN 和 JSON 的数据形状开放，普通 checked 解析保留 `Result<Dynamic,String>`。业务代码需要闭合名义类型时，在边界使用 `try-parse-cirru-edn-as` 或 `decode-map-as` 验证，不把开放 payload 直接当作已证明类型。

`format-cirru` 接受以 List 表示的 Cirru 顶层行，内部节点仍可递归包含 String/List；传入 Number 等非 List 会在严格检查阶段报类型不匹配。它与格式化任意可序列化数据的 `format-cirru-edn` 不是同一个契约。

`format-cirru-one-liner` 也要求外层 List，格式化单个 Cirru 表达式；Number 等非 List 会在严格检查阶段报类型不匹配。内部 String/List 节点保持开放，不能据此外推为任意 Calcit 值格式化接口。

Under the default strict diagnostics, passing that open `Dynamic` result directly to a
function argument whose contract contains a Struct or Enum is
`E_DYNAMIC_NOMINAL_ARGUMENT`. This also covers matching containers such as
`List<Dynamic>` entering `List<Person>`. Decode at the boundary instead of
letting application code accidentally treat an open map as a nominal value;
`--compat-types` retains the warning path while a codebase migrates.

These Result-returning parser methods currently run on the native and JavaScript backends. The WASM backend does not yet support the underlying parser procedures or `try`-based wrappers; the standard WASM validation suite confirms that unsupported parser definitions are skipped without affecting supported exports.

## Strict typed decoding

`parse-cirru-edn-as` is a language syntax that derives a closed decoder graph at compile time, then validates and constructs the complete value recursively:

公开定义需要声明可检查的函数契约；以下 `hint-fn` 与 Snapshot `Fn` schema 等价地提供这一信息：

```cirru.no-run
def Person $ defstruct Person (:name 'String) (:age 'Number)

defn decode-person (raw)
  hint-fn $ {}
    :args $ [] 'String
    :return 'Person
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
  hint-fn $ {}
    :args $ [] 'String
    :return $ :: 'Result (:: 'List 'Person) 'String
  try-parse-cirru-edn-as raw $ :: 'List Person
```

Its inferred return type is `Result<List<Person>,String>`, not `Result<Dynamic,String>`. Runtime parse and recursive shape failures become `:err` with the same structural context. An invalid `TypeExpr` remains a compile-time error because it is a program definition problem rather than recoverable input.

`TypeExpr` 中的名义类型名按所在 namespace 的 import 解析，`parse-cirru-edn-as`、`try-parse-cirru-edn-as`、`decode-map-as` 与 `try-decode-map-as` 共用这一规则：`:as` 别名（`schema/External`）、`:refer` 导入的短名（`External`）和完整路径（`app.schema/External`）指向同一个声明，解码出的值也相同；不同 namespace 中同名的类型各自保持独立身份。别名未导入或目标定义缺失时，在编译期报 `cannot resolve named type`，不会退化为 Dynamic。

## Decoding runtime maps into Structs

`decode-map-as` is the companion for data that has already crossed a host boundary and is now an evaluated Calcit value, such as a JSON object returned by a JavaScript FFI. It derives the target shape at compile time and returns the declared type, so it is the typed replacement for ad-hoc map readers and runtime schema libraries. Struct targets consume maps, while list, map, enum, ref, and scalar targets are decoded recursively as well.

Decode exactly once, at the boundary. Passing an already nominal Struct to
`decode-map-as` or `try-decode-map-as` is a compile-time
`E_DECODE_MAP_AS_ALREADY_STRUCT` error; use that Struct directly. This prevents
an updater or history loader from accidentally re-decoding typed state and
failing later with “expected map, got struct”.

它会递归地把 Map 解码为 nominal Struct，拒绝未知 key 与缺失的必填字段，并报告错误值所在路径。缺失的 `Option<T>` 字段变成 `Option :none`，已有的原始 `T` 变成 `Option :some value`。旧 helper 创建的等价值在迁移窗口内仍可读取。

```cirru.no-run
defstruct Response (:code 'Number)
  :message $ :: 'Option 'String
  :body 'Dynamic

; `raw` may be { :code 200, :message |ok, :body ... }
; the result is Response(:code 200, :message (Option :some |ok), :body ...)
; omitting :message produces (Option :none)
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

---
title: "Error Handling"
scope: "core"
kind: "guide"
category: "features"
aliases:
  - "try raise"
  - "exception handling"
  - "errors"
---

# Error Handling

Calcit uses `try` / `raise` for exception-based error handling. Errors are string values (or tags) propagated up the call stack.

## Quick Recipes

- **Catch Errors**: `try (risky-op) (fn (e) ...)`
- **Throw String**: `raise |something-went-wrong`
- **Throw Tag**: `raise :invalid-input`
- **Match Error**: `if (= e :invalid-input) ...`

## Basic `try` / `raise`

`try` takes an expression body and a handler function. If the body raises an error, the handler receives the error message as a string:

Native and generated JavaScript normalize caught failures to this String contract; host `Error` objects are not exposed through the handler.

```cirru
let
    result $ try
        raise |something-went-wrong
      fn (e)
        str-spaced |caught: e
  println result
  ; => caught: something-went-wrong
```

## `try` 的类型推断

`try` 合并正常表达式与错误处理器的返回类型。错误处理器收到 `String`；未标注的匿名函数可从这里获得参数类型，返回类型仍从函数体独立推断。两边都是 `Bool` 时结果为 `Bool`；`Option :some` / `Option :none` 和 `Result :ok` / `Result :err` 使用现有的名义类型分支合流规则。

```cirru
let
    result $ try (Result :ok 7)
      fn (message)
        Result :err message
  assert-type result $ :: 'Result 'Number 'String
```

正常分支直接 `raise` 时只有处理器产生值；处理器直接 `raise` 时只保留正常值类型。处理器表达式仍然仅在错误发生后求值，类型推断不执行处理器，也不改写求值顺序。

限制：未知或 `Dynamic` 返回值仍需在真实边界解码；外层返回声明不能替代实现证明，也不会授予处理器 `:js-ffi` 权限。本规则覆盖 native 与 JavaScript；不新增 WASM 的 `try` 支持。

## Raising from a Function

```cirru
let
    safe-div $ fn (a b)
      if (= b 0)
        raise |division-by-zero
        / a b
    result $ try
        safe-div 10 0
      fn (e)
        str-spaced |error: e
  println result
  ; => error: division-by-zero
```

## Raising Tags as Error Codes

Tags are a clean way to represent error categories:

```cirru
let
    validate-age $ fn (n)
      if (< n 0)
        raise :negative-age
        if (> n 150)
          raise :unrealistic-age
          n
    result $ try
        validate-age -5
      fn (e)
        str-spaced |validation-failed: e
  println result
  ; => validation-failed: :negative-age
```

## Silent Success vs Error Paths

When an error is raised, execution jumps to the handler — intermediate values are not returned:

```cirru
let
    might-fail $ fn (flag)
      if flag (raise |early-exit) 42
    a $ try (might-fail false) $ fn (e) -1
    b $ try (might-fail true) $ fn (e) -1
  println a
  ; => 42
  println b
  ; => -1
```

## Nested `try`

Inner `try` handlers can re-raise or recover selectively:

```cirru.no-check
try
  try
      risky-operation
    fn (e)
      if (= e :recoverable)
        default-value
        raise e
  fn (outer-e)
    log-error outer-e
    nil
```

## Using Enums for Typed Results (Preferred Pattern)

Instead of exceptions, idiom Calcit code often uses a `Result` enum to represent success/failure without throwing:

```cirru
let
    AppResult $ defenum AppResult (:ok :number) (:err :string)
    safe-compute $ fn (x)
      if (> x 0)
        %:: AppResult :ok (* x 10)
        %:: AppResult :err |negative-input
    handle $ fn (r)
      match r
        (:ok v)
          str-spaced |result: v
        (:err msg)
          str-spaced |failed: msg
  println $ handle (safe-compute 5)
  ; => result: 50
  println $ handle (safe-compute -1)
  ; => failed: negative-input
```

This pattern avoids exceptions entirely and keeps error handling explicit in the type system.

## Assertions

`assert` and `assert=` raise errors during preprocessing/testing:

```cirru.no-check
; "assert a condition is true"
assert (> x 0) |expected-positive

; "assert two values are equal"
assert= (+ 1 2) 3
```

`assert-type` checks type at preprocessing time:

```cirru.no-check
; "assert x is a number before using it"
assert-type x :number
```

## Marking unfinished paths with `todo!`

Use the compiler-known `todo!` expression for an intentionally unfinished
implementation, especially in scaffold-generated definitions:

```cirru.no-check
defn validate-order (order)
  todo! "|implement order validation"
```

`todo!` accepts zero or one static String message. Static preprocessing emits a
`W_TODO` warning with the definition and path, so normal check/codegen gates
continue to report unfinished work. Reaching it at runtime raises a TODO effect;
JavaScript throws an Error and WASM traps. It is not an alias for `raise`:
`raise` is an ordinary application error path and does not mean that an Agent
still needs to implement the code.

## Notes

- `raise` accepts any value that can be converted to a string. String literals and tags work best.
- Raising maps or complex data structures may produce unexpected results — use the Result enum pattern for structured error data.
- `try` always produces a value: either the result of the body, or the result of the handler.
- `assert` / `assert=` are for development-time invariants. They generate warnings (not runtime errors) during static analysis.
- `todo!` is a completion warning and should be removed when the implementation is finished; do not use it for recoverable domain errors.

## See Also

- [Enums](enums.md) — Result/Option patterns for typed error handling
- [Static Analysis](static-analysis.md) — `assert-type`, type hints
- [Common Patterns](common-patterns.md) — defensive programming examples

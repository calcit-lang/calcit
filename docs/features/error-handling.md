# Error Handling

Calcit uses `try` / `raise` for exception-based error handling. Errors are string values (or tags) propagated up the call stack.

## Quick Recipes

- **Catch Errors**: `try (risky-op) (fn (e) ...)`
- **Throw String**: `raise |something-went-wrong`
- **Throw Tag**: `raise :invalid-input`
- **Match Error**: `if (= e :invalid-input) ...`

## Basic `try` / `raise`

`try` takes an expression body and a handler function. If the body raises an error, the handler receives the error message as a string:

```cirru
let
    result $ try
        raise |something-went-wrong
      fn (e)
        str-spaced |caught: e
  println result
  ; => caught: something-went-wrong
```

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
      tag-match r
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
; assert a condition is true
assert (> x 0) |expected-positive

; assert two values are equal
assert= (+ 1 2) 3
```

`assert-type` checks type at preprocessing time:

```cirru.no-check
; assert x is a number before using it
assert-type x :number
```

## Notes

- `raise` accepts any value that can be converted to a string. String literals and tags work best.
- Raising maps or complex data structures may produce unexpected results — use the Result enum pattern for structured error data.
- `try` always produces a value: either the result of the body, or the result of the handler.
- `assert` / `assert=` are for development-time invariants. They generate warnings (not runtime errors) during static analysis.

## See Also

- [Enums](enums.md) — Result/Option patterns for typed error handling
- [Static Analysis](static-analysis.md) — `assert-type`, type hints
- [Common Patterns](common-patterns.md) — defensive programming examples

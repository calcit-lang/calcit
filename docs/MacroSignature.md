# Phase-aware macro signatures

Calcit macros run in two phases: their parameters are unevaluated syntax, while
their expansion is later preprocessed as an expression or declaration. A macro
signature therefore uses a dedicated `MacroSignature`; it is not a function
signature with a different `:kind`.

The canonical snapshot/source form is:

```cirru
:: 'Macro
  {} (:generics $ [] 'T)
    :required $ [] 'SyntaxSymbol (:: 'Expr 'T)
    :optional $ [] 'SyntaxList
    :rest 'Syntax
    :expansion $ :: 'Expr 'T
```

Input contracts are `Syntax`, `SyntaxSymbol`, `SyntaxList`, and `Expr<T>`.
`SyntaxSymbol` and `SyntaxList` inspect the raw AST shape. `Expr<T>` accepts an
expression syntax node and, where its type is inferable at the call site, binds
or checks its semantic type. Optional slots are declared separately from
required slots, and `:rest` describes each additional input node; the macro body
receives that rest binding as a syntax list.

Expansion contracts are `Expr<T>`, `Definition<T>`, and `Declarations`.
Expression contracts are checked after expansion and preprocessing. Definition
contracts additionally require definition syntax, while `Declarations` accepts
one declaration or a `do` form containing declarations. Diagnostics identify
whether the violation belongs to input syntax or the expansion result and keep
the macro call stack and source location.

Existing `(:: 'Macro ({} (:args ...) (:return ...)))` schemas remain readable
and serialize without data loss. They become an explicitly legacy, non-strict
`MacroSignature`: their runtime-looking types are compatibility metadata, not
syntax contracts. An omitted or `Dynamic` legacy macro schema likewise does not
claim full phase coverage.

Two motivating cases illustrate the distinction:

- Core `%{}?` receives a struct name as `SyntaxSymbol`, then a rest sequence of
  `SyntaxList` field entries, and expands to `Expr<Struct>`.
- Respo `defstyle` receives a style name as `SyntaxSymbol` and rule data as
  `SyntaxList`; its expansion is a definition/declaration contract rather than
  a runtime function return value.

This model intentionally does not introduce recursive runtime union types,
macro expansion caches, or compile-time capability policy.

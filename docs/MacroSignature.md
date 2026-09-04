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
    :capabilities $ #{}
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
and serialize without data loss. Current Calcit no longer accepts such runtime
`Fn` schemas or an omitted/whole-`Dynamic` macro schema: Snapshot loading fails
with the exact definition path. For an already structured `CodeEntry`, migrate
the module using final-compatible Calcit 0.13.51, then publish its strict
contract before upgrading consumers. An earlier direct-quote Snapshot instead
uses the current isolated `calcit edit format` path: the formatter preserves its
parameter shape as `Syntax`, emits `Expr<Dynamic>`, and grants no capabilities.
This conservative bridge is limited to definitions decoded from direct quotes;
it neither rewrites an existing Dynamic schema nor guesses semantic types or
compile-time effects. Review and narrow the generated contract before release.
Always include `--deps` when checking an application, since installed releases
are what the loader resolves.

## Migrating structural macros

Do not replace a whole-`Dynamic` schema with invented runtime parameter types.
First describe the raw syntax the macro actually inspects and the semantic kind
of the emitted form. For example, a binding macro with one list-shaped syntax
argument and arbitrary body forms can use:

```cirru
:: 'Macro
  {} (:required $ [] 'SyntaxList)
    :rest 'Syntax
    :expansion $ :: 'Expr 'Dynamic
    :capabilities $ #{}
```

`Expr<Dynamic>` here is an explicit semantic boundary: the expansion is known
to be an expression, but its value type depends on user code. It is stricter
than a legacy whole-`Dynamic` macro without pretending that every body form has
one static value type. Use `SyntaxSymbol` for declaration names, `SyntaxList`
for argument/binding/pair lists, `Expr<T>` only when call-site semantic typing
is meaningful, and `Definition<T>` or `Declarations` only when the emitted AST
is genuinely a definition.

After migration, inspect the stored contract rather than relying on coverage
percentages alone:

```bash
calcit query schema calcit.core/let
calcit query context calcit.core/let --format json
```

The bundled core snapshot has an additional inventory gate: every `defmacro`
in `calcit.core`, `calcit.internal`, and `calcit.test` must load as a strict
phase-aware signature. Adding a new bundled macro with an omitted or legacy
whole-`Dynamic` schema fails the Rust test suite. This complements runtime
metrics, which only report macros reached by the selected entry.

Before promoting an experimental macro, measure its source adoption in the
Calcit workspace rather than relying on its presence in documentation or the
core definition itself:

```bash
rg --hidden --follow -n -F \
  --glob '*.cirru' \
  --glob '!**/.git/**' \
  --glob '!**/src/cirru/calcit-core.cirru' \
  -- 'MACRO_NAME' /path/to/workspace
```

`--hidden --follow` includes sources reached through project-local
`.calcit/modules/` links. Use the literal matches as an audit candidate list,
then manually distinguish executable call sites from definitions, docs,
comments, quoted code, and longer symbols. Record the executable call sites in
the promotion PR. Zero-call-site syntax should normally stay in a module or be
removed instead of becoming permanent core surface.

Some strict contracts still contain intentional `Dynamic` positions. Broad
collection helpers such as `first`, `nth`, and `get` accept several runtime
shapes that cannot yet be expressed as one static union or trait contract.
Their convenience macros therefore describe argument count and expression
phase precisely while retaining `Expr<Dynamic>` for the open payload. More
specific boundaries should remain concrete: for example, `js-object` expands
to `JsObject`, and anonymous enum
sugar expands to `Enum`.

## Compile-time capabilities

A strict macro is pure by default. Effects performed while its body is being
evaluated must be declared separately from runtime/backend `:features`:

```cirru
:: 'Macro
  {} (:required $ [])
    :expansion $ :: 'Expr 'String
    :capabilities $ #{} :env-read :fs-read
```

Allowed opt-in capabilities are `:env-read`, `:fs-read`, `:platform-read`,
`:clock-read`, `:log`, `:mutable-state`, and `:dynamic-eval`. `:log` covers
compile-time calls to `echo`, `println`, and `eprintln`; merely emitting those
calls into quoted runtime syntax stays pure. Any declared capability
makes the expansion ineligible for the pure-macro cache planned by the macro
roadmap. Legacy signatures have unknown effects and are likewise ineligible.

`:fs-write`, `:process`, and `:host-ffi` are represented in the capability
model for diagnostics and auditing, but are rejected during macro expansion
even when declared. Native methods, JS/raw host access, and registered host
procedures are all treated as host FFI rather than as unclassified pure calls.

Capability checks remain active through ordinary helper functions. A missing
declaration reports `E_MACRO_CAPABILITY_MISSING`; a forbidden capability reports
`E_MACRO_CAPABILITY_DISALLOWED`. Diagnostics include the macro, operation,
call-site location, and helper chain.

Only effects executed during expansion need capabilities. A pure macro may
quote or construct syntax containing `get-env`, `read-file`, state mutation, or
other runtime effects: those operations are checked only if the macro evaluator
actually calls them while deciding the expansion.

Two motivating cases illustrate the distinction:

- Core `%{}?` receives a struct name as `SyntaxSymbol`, then a rest sequence of
  `SyntaxList` field entries, and expands to `Expr<Struct>`.
- Respo `defstyle` receives a style name as `SyntaxSymbol` and rule data as
  `SyntaxList`; its expansion is a definition/declaration contract rather than
  a runtime function return value.

This model intentionally does not introduce recursive runtime union types or
macro expansion caches. It exposes cache eligibility for the later expansion
optimization without caching anything yet.

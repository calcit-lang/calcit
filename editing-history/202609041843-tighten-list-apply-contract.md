# Tighten List.apply contracts

`&list:apply` previously exposed `List<T> × List<Fn> -> List<U>`. The erased function shape did not connect the receiver member type to callback inputs, and `U` appeared only in the return position. The core schema now requires every function to share `T -> U`, with an attached positive test proving the public method returns `List<String>` for homogeneous transformations.

The fallback method checker also skipped binding the receiver against the first schema argument. That allowed a later argument to choose `T` independently of the receiver. It now seeds generic bindings from the receiver before checking method arguments, reports the stable `W_METHOD_ARG_TYPE_MISMATCH` code, and renders the substituted nested expected type. Direct `&list:apply` calls receive a targeted expected-function specialization and both direct and method diagnostics include an actionable normalization-or-splitting migration hint.

An initial experiment applied accumulated generic substitutions eagerly to every user-function argument. That changed established `Option` fold behavior by rejecting its intentional `nil` lifting, so the broad change was discarded in favor of the targeted `List.apply` specialization.

Regression coverage includes positive core execution and negative direct/method calls whose `Number -> Number` functions are applied to `List<String>`. Validation includes canonical Snapshot formatting, Rust formatting, strict Clippy, serialized Rust tests, core quality/dynamic classification, and the repository-wide `yarn check-all` gate.

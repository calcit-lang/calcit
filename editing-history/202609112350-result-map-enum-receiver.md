# Check resolved Result enum receivers

- Extend the checked `result:map` contract to accept resolved `calcit.core/Result<T,E>` enum values as well as source-level type references.
- Preserve nominal identity so user-defined enums named `Result` do not acquire the core contract.
- Cover constructor-produced Result locals and their callback input, output, and error type relations.

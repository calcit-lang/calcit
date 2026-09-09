# Reject implicit Option/Result stringification in strict mode

- Added the stable `E_NOMINAL_ENUM_STRINGIFICATION` preprocessing error when project code passes a statically known core `Option<T>` or `Result<T,E>` directly to `str` or `turn-string`.
- Kept ordinary values, unrelated application enums, explicit `to-lispy-string` representation, core implementation code, and `--compat-types` behavior unchanged.
- Covered public `str`, direct `turn-string`, variadic argument positions, Option and Result guidance, application-defined enum names, and an end-to-end strict preprocessing regression.
- Documented the explicit payload-handling and diagnostic-representation migration paths.

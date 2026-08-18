## Review follow-up: core Option identity

- Omitted-argument sugar is reserved for source references to `calcit.core/Option`; resolved enum values do not retain a namespace and therefore cannot be safely treated as the core type.
- A user-defined enum named `Option`, including one with `some` and `none` variants, keeps exact arity and never receives an implicit `%none` argument.
- Documentation examples use callable enum constructors: `(%none)` and `(%some value)`.

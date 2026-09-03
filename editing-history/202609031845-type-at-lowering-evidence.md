# Type-at preprocess lowering evidence

`query type-at` previously exposed inferred types and method candidates, but it did not show whether preprocessing had actually converted rich source syntax into a direct core operation. This made type coverage an incomplete proxy for runtime dispatch cost and gave migration tools no stable way to distinguish successful specialization from an ordinary symbol lookup.

The v2 response now includes `data.lowering` with a closed status, a lowering kind, source and lowered callable heads, and a human-readable explanation. The classifier deliberately separates type-directed collection specialization, nominal Struct/Enum construction and access, static methods, and typed external operations from ordinary static call resolution. A known receiver that still uses method dispatch is reported as `dynamic`, because type evidence alone does not prove direct execution.

The report is derived from the source-correlated preprocessed node already used by type inference. It does not evaluate the application or add runtime instrumentation. A missing correlated node is explicit as `unavailable`, which is important for rewrites that fold an expression into a location-free value or for preprocessing failures.

Validation covers synthetic generic-to-primitive lowering, direct primitive calls, unresolved dynamic methods, and a real typed Struct field compiled from `calcit/test.cirru`. The CLI output was also checked to report `:x -> &struct:nth` for `test-struct.main/sum-point`.

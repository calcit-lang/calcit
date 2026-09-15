# Component numeric contracts

- Interface IR v3 and Component Interface IR v2 export every width-specific numeric refinement as a distinct scalar `kind`; plain `Number` remains `number`.
- One recursive `FfiTypeIr` conversion covers function signatures, nominal declarations, and nested List/Option/Result positions, so consumers never infer widths from names or values.
- Numeric kinds participate in the existing deterministic revision payload. Changing `Number` to `Float64` therefore changes the ABI fingerprint even though both use the runtime f64 representation.
- The current exporter replaces the preview v2/v1 protocols directly. Their schema files remain only as historical references; no compatibility writer or fallback reader is maintained while the Component ecosystem remains experimental.
- Verification: `cargo test`, `cargo clippy -- -D warnings`, `yarn compile`, `yarn check-agent-interface`.

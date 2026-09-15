# Component diagnostic protocol version

- Component diagnostics are projected from the shared Interface IR conversion path.
- Rewrite protocol-version references in both `message` and `suggestion`; rewriting only the message leaves a contradictory machine-readable diagnostic.
- Verification: `cargo test ffi_interface_ir::tests --lib` and `cargo clippy -- -D warnings`.

# Same-package namespace loading

- Treat namespaces from the target package as already authoritative when a transitive dependency cycle lists them again.
- Keep rejecting conflicting namespaces that cross package boundaries.
- Add regression coverage for duplicate same-package namespaces with different snapshot content.

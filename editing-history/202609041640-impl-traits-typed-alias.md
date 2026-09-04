# Preserve typed `impl-traits` alias evidence

- Keep an explicit function return schema authoritative for nominal fields and generic arguments.
- Recover attached Struct impl evidence from the preprocessed function body's guaranteed return paths when the body constructs the same nominal type through a top-level `impl-traits` alias.
- Ignore explicitly diverging `raise` branches, while refusing to merge different live alias attachments.
- Cover the nominal match, field preservation, method evidence, divergence, and ambiguous-branch cases with focused Rust tests.
- Verify the real `calcit-lang/calcit.std@c3f91a1` consumer with `calcit calcit.cirru --check-only`, including its prefix and receiver-first Date method calls and typed field access.

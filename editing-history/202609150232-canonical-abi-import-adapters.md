# Canonical ABI import adapters

- Keep raw Component imports in Canonical ABI shape and route Calcit call sites through generated internal-value wrappers; do not expose Calcit's tagged heap layout to bindgen or hosts.
- A `Number` remains `f64`. A String parameter borrows the internal UTF-8 payload as `(ptr, len)` for the duration of the imported call.
- A synchronous imported String result has more than one flat result, so `canon lower` appends a caller-provided `i32` return-area pointer to the raw import signature. The adapter allocates eight bytes, reads `(ptr, len)`, and copies the bytes into a tagged Calcit String.
- Component modules contain only explicit typed imports. Native core modules retain the legacy host table, and component validation continues to reject unsupported schemas without a Dynamic fallback.
- The two literal strings in a `defwasm-import` declaration are module/symbol metadata, not an executable function body. Strict preprocessing validates the declared function schema but must not infer a return type from those metadata strings.
- Keep user-visible Calcit semantics in definition tests where possible; raw import signatures, return-area layout, and host linking remain low-level integration coverage.

# Omit match wildcard bindings in JavaScript output

- Treat `_` in enum match payload positions as a true wildcard in both generic and indexed JavaScript match emitters.
- Do not emit a JavaScript `let` declaration or add `_` to the lexical scope for wildcard positions.
- Cover repeated wildcards so generated modules remain parseable by Vite, Rollup, and native JavaScript engines.

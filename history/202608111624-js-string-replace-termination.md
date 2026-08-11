## JS string replacement termination

- `&str:replace` must replace each original literal match exactly once; repeatedly replacing until the pattern disappears loops forever when the replacement contains the pattern.
- Escape the literal pattern and use a global callback replacement so replacement text remains literal too, including `$` sequences.
- Keep empty-pattern behavior aligned with Rust `str::replace`, and cover self-containing replacements, regex metacharacters, literal replacement text, and empty patterns in the JS runtime regression script.

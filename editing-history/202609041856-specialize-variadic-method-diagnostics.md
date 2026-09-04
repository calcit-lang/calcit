# Specialize variadic method diagnostics

Review of the receiver-binding change found that fixed method arguments used substituted, fully described expected types while variadic method arguments still rendered the unspecialized brief annotation. A generic method such as `Map<K,V>.merge(...Map<K,V>)` could therefore warn that a `map` was received where a `map` was expected.

Variadic method checking now substitutes receiver-derived bindings before matching each rest argument and uses full type descriptions for both sides of the diagnostic. The collection mismatch fixture adds a `Map<Tag,Number>.merge(Map<Tag,Tag>)` call and asserts the stable warning reports `map<tag, number>` versus `map<tag, tag>`.

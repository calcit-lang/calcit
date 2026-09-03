# Lower typed Option access before runtime

The public `get`, `nth`, and `first` APIs deliberately return nominal `Option<T>`, but their core implementations historically selected Map/List/String/Enum behavior through runtime type predicates. Static receiver evidence was already sufficient to infer the result type and resolve receiver-style methods, yet code generation still retained the generic branching implementation.

Preprocessing now expands these calls when the receiver selects one concrete collection family. Typed Map lookup becomes `&map:contains?` plus `&map:get`; typed List/String/Enum indexing becomes a family-specific bounds check and nth primitive; List/String `first` uses their direct empty/first primitives, while Enum uses count/nth. `%some` and `%none` remain the public absence contract, so this optimization does not reintroduce nil sentinels.

Generated hygienic `let` bindings preserve left-to-right, exactly-once evaluation of receiver, key, and index expressions. The expansion is applied to both prefix compatibility calls and the preferred receiver-style `.get`, `.nth`, and `.first` syntax. Dynamic receivers stay on the generic compatibility path instead of being guessed into a concrete shape.

The second preprocess pass is intentionally limited to those three resolved core callables. Reprocessing every statically inlined method would require unrelated custom nominal callables to be present in the program registry and would change the established direct-Import contract.

Regression coverage checks primitive selection, absence of receiver predicates, exactly-once source expressions, Dynamic fallback, native behavior, generated JavaScript behavior, and the absence of `invoke_method` for typed access. The List and HashMap guides now use receiver-style access and show the actual Option results.

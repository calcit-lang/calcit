> some notes about evaluating symbols

There several kinds of symbols:

- raw syntax symbols, `&` `?` `~` `~@`...
- data symbol, probably created via `turn-symbol`
- local variables
- local definitions
- imported variables
- namespaced imported symbols
- imported default variables
- imported host variables

Currently they are share the structure `Calcit::Symbol{..}`, which is buggy
and requires refactor in future.

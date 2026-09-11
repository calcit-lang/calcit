# Explain expression-shaped search-replace patterns

- Distinguish expression-shaped patterns from genuinely absent leaf patterns when `tree search-replace` finds no match.
- Return stable diagnostic codes, state that no changes were written, and provide a copyable `query search-expr --set-cursor` recovery path.
- Preserve regex leaf matching and cover expression, missing-leaf, and regex cases.

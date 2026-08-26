# Keep `flipped` compatible with host callables

- Relaxed `flipped`'s first macro input from source schema `Fn` (represented
  internally as `DynFn`) to `Dynamic`.
- The macro only reverses call syntax; JavaScript host symbols such as
  `js/setTimeout` are intentionally dynamic and cannot satisfy the native
  callable schema before host lowering.
- Kept the rest arguments and expansion result explicitly dynamic, while the
  macro remains compile-time pure and phase-aware.
- Validated the change against current Respo UI usage, which relies on
  `flipped js/setTimeout ...`.

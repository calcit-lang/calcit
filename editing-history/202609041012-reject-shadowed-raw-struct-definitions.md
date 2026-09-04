# Reject shadowed raw Struct definitions

- Made evidence-complete persisted `&%{}` checks consult lexical scope before
  resolving a namespace-local constructor symbol.
- A local binding that shadows a global `defstruct` can no longer borrow the
  global layout as proof for raw constructor IR.
- Added a focused strict regression alongside the valid Snapshot symbol case.

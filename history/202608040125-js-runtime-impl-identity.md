# JS runtime impl identity

- Fixed #293 by branding `CalcitImpl` values with a shared `Symbol.for` key, so duplicate `@calcit/procs` module instances agree on impl identity.
- Re-interned cloned impl field tags in the receiving runtime to avoid the same cross-module identity problem during `castTag`.
- Added a Node regression check that loads two complete runtime copies and promotes an impl value across them.
- Verified the patched runtime in a temporary Timegrass snapshot with Node 24 and Vite 8.

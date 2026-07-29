# Edit target cursor alias

- Added `@cursor` definition targets to definition, metadata, copy/move, rename, and split operations.
- Defined the alias as the source for two-target commands while keeping destinations explicit.
- Reused a single active target/path resolution when an edit references both.
- Left transaction operation files self-contained with concrete targets and paths.

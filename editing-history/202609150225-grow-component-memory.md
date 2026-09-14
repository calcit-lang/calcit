# Grow Component memory on demand

- `cabi_realloc` must grow memory for ordinary allocations that exceed the initial core-module memory instead of treating them as invalid input.
- Compute missing pages in `i64` so rounding up cannot overflow the memory32 address range, and preserve the special 65,536-page case where the full 4 GiB is already present.
- Node-based integration checks must turn rejected promises into a nonzero process exit; otherwise some Node configurations can report a false pass.
- Keep documentation explicit about the currently shipped `Number`/`String` adapter slice versus the broader planned synchronous type closure.

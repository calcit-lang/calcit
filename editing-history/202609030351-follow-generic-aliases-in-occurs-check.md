# Follow generic aliases in the occurs-check

## Context / 背景

Review of calcit-lang/calcit#597 identified that checking only the candidate
annotation catches direct bindings such as `T = Optional<T>`, but not indirect
cycles through existing aliases.

对 calcit-lang/calcit#597 的 review 指出，仅检查候选 annotation 能阻止直接
递归绑定，但仍可能通过已有别名形成间接环。

## Change / 修改

- Make the occurs-check traverse existing generic bindings with a visited set.
- Keep the permissive unresolved match while refusing to store cyclic edges.
- Cover `T = U` followed by a rejected `U = Optional<T>` edge.
- Cover the reverse `T` versus `Optional<T>` matching direction and verify a
  later concrete binding still succeeds.

Validation: `cargo fmt`, strict Clippy, `yarn compile`, full Rust tests,
`yarn check-all`, and `yarn check-agent-interface`.

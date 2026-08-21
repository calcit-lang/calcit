# Type system ecosystem RFCs

- Added a setup-cr proposal that makes `deps.cirru` the normal single source
  of truth for the Calcit version. Explicit action input remains a fallback;
  conflicting sources fail instead of silently overriding each other.
- Defined a native type-quality CI adoption model around `analyze quality`,
  per-definition baselines, cumulative Q0-Q4 evidence, and explicit backend
  runtime tests. Project-specific JavaScript report aggregators are rejected.
- Extended the existing typed JS FFI design with runtime contract evidence,
  reusable guards/decoders, Node/browser negative fixtures, and auditable
  unsafe host assertions.
- Added a long-term static type-system roadmap inspired by Rust and MoonBit:
  distinguish Unknown from intentional Dynamic, improve exhaustiveness,
  narrowing and bidirectional local inference, and type framework boundaries
  before considering more complex trait or effect features.
- Replaced the obsolete fixed-version GitHub Actions quick start with a
  `deps.cirru`-driven workflow, and added `setup-cr`/quality-gate metadata so
  `cr docs search` points users to the relevant CI and upgrade guidance.
- Removed the RFC index entry for a non-existent project tooling RFC and
  replaced it with the concrete toolchain contract proposal.

Validation: Markdown structure and Calcit fenced examples are checked with
`cr docs format-md --check` and `cr docs check-md`; `cr docs search` resolves
the new setup-cr and quality-gate entry; repository diff is checked for
whitespace errors.

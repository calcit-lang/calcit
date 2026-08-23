# `caps verify --toolchain` runtime version guard

- `deps.cirru :calcit-version` is the project-selected Calcit release. A regular `caps verify` validates module-store integrity; `caps verify --toolchain` additionally makes this version a CI gate.
- For JavaScript projects the guard requires the running `caps` release, `package.json`'s `@calcit/procs` anchor (either the exact version or `^` with the same anchor), and the version Yarn resolves through `yarn node` to match exactly. This supports both Yarn PnP and node-modules installations.
- `caps upgrade --all` now requests `@calcit/procs@^<current-calcit-version>` explicitly, so its manifest update remains tied to the Calcit release rather than whichever npm version happens to be latest.
- Place `caps verify --toolchain` after `caps --ci && yarn install --immutable` in JS CI. Native-only projects without `package.json` remain supported and verify the Calcit/deps pair only.

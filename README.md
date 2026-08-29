### Calcit Programming Language

> A typed functional language for interactive and real-time applications. Built with Rust and compiling to JavaScript ES Modules.

- Home https://calcit-lang.org/
- API Doc https://apis.calcit-lang.org/
- Guidebook https://guide.calcit-lang.org/

[Browse examples](https://github.com/calcit-lang/calcit/tree/main/calcit) or also [try WASM version online](https://github.com/calcit-lang/calcit-wasm-play).

Core design:

- Interpreter runs on Rust, extensible with Rust FFI
- Persistent Data Structure
- Indentation-based Cirru syntax, friendly to plain text editing
- Code-as-data macros and functional style
- Nominal structs/enums, traits, methods, Option/Result, and static analysis
- Compiles to JavaScript in ES Modules, JavaScript Interop
- Hot code swapping friendly

Current direction:

- `calcit.cirru` is the canonical source snapshot; retired `compact.cirru` inputs receive migration guidance
- CLI-first development with `calcit` and `caps`, designed to work well with AI agents in terminal workflows
- Better CLI editing and validation for CI, docs lookup, module management, and incremental updates
- Consistent support for real-time web applications: typed WebSocket messages, deterministic state updates, diff/patch synchronization, acknowledgement, and resynchronization

### Install ![GitHub Release](https://img.shields.io/github/v/release/calcit-lang/calcit)

Build and install with Rust:

```bash
# get Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# get Calcit user-facing tools
cargo install calcit --bin calcit --bin caps
```

Installed binaries:

- `calcit`, the runtime and JS compiler
- `caps`, for downloading dependencies declared in `deps.cirru`

When installing from source, install the same public tools:

```bash
cargo install --path . --bin calcit --bin caps
```

For new GitHub Actions workflows, use [setup-calcit@v1](https://github.com/calcit-lang/setup-calcit). It
installs `calcit` and creates a lightweight `cr -> calcit` compatibility link for existing workflow commands;
for pre-rename releases it falls back to `cr` and exposes the equivalent `calcit` command.
Local installations ship only `calcit`; migrate local scripts directly instead of relying on a wrapper. Existing
[setup-cr](https://github.com/calcit-lang/setup-cr) workflow tags remain supported.

### Quick Start

Evaluate snippets:

```bash
calcit eval 'range 100'

calcit eval -- '-> 100 range (map $ \ * % %)'
```

Run with the canonical runtime snapshot `calcit.cirru`:

```bash
calcit calcit.cirru # run once (default)
calcit # reads calcit.cirru from the current directory

calcit -w # watch mode (explicit flag required)
```

By default Calcit reads `:init-fn` and `:reload-fn` from `calcit.cirru` `:entries.default`. You may also specify functions:

```bash
calcit --init-fn='app.main/main!' --reload-fn='app.main/reload!'
```

You may also configure `:entries` in `calcit.cirru`:

```bash
calcit --entry server
```

### JavaScript codegen

Calcit compiles to JavaScript with consistent semantics. In browser or Node projects, JavaScript interop is still expected.

```bash
calcit js # compile to js, also picks `calcit.cirru` by default
calcit js --emit-path=out/ # compile to js and save in `out/`
```

By default, js code is generated to `js-out/`. You will need Vite or Node to run it, from an entry file:

```js
import { main_$x_, reload_$x_ } from "./js-out/app.main.mjs";
main_$x_(); // which corresponds to `main!` function in calcit
```

### CLI and Agent Workflow

The recommended workflow is plain text editing plus CLI validation, often driven by an AI agent in terminal.

Common commands:

```bash
calcit docs agents --full   # read the current agent workflow guide
calcit query search 'foo'   # locate code by symbol or string
calcit edit ...             # structured edits for defs, imports, config, modules
calcit js                   # compile once
calcit js -w                # watch mode
caps                    # install/update dependencies from deps.cirru
```

Calcit Editor is no longer the recommended path for everyday development. If you still need the older editor workflow, see [Calcit Editor](https://github.com/calcit-lang/editor).

Related examples and workflows:

- [Minimal Calcit](https://github.com/calcit-lang/minimal-calcit/blob/main/README.md)
- [Respo Calcit Workflow](https://github.com/calcit-lang/respo-calcit-workflow)
- [setup-calcit](https://github.com/calcit-lang/setup-calcit) for new GitHub Actions workflows

### Modules

`deps.cirru` declares dependencies that need to download, which correspond to repositories on GitHub. Specify a branch or a tag:

```cirru
{} (:calcit-version |0.9.11)
  :dependencies $ {} (|calcit-lang/memof |0.0.11) (|calcit-lang/lilac |main)
  :dev-dependencies $ {} (|calcit-lang/calcit-test |0.1.0)
```

Run `caps` to resolve the recursive dependency graph and install it. Immutable revisions are stored under
`~/.config/calcit/module-caches/`, while the current project receives links under `.calcit/modules/`.
Different projects can therefore use different revisions without switching a shared checkout. Existing
project module links are the only runtime source for package-style module paths; explicit relative and
absolute paths retain their normal direct resolution.

Published SemVer tags are preferred. Branch refs remain supported for development, but `caps` warns with
the resolved commit. When a graph requests several SemVer tags for one repository, the highest requested
version is selected and reported.

Root projects install both `:dependencies` and `:dev-dependencies`. Recursive resolution only follows
`:dependencies`, so test and maintenance modules declared by a dependency do not leak into consumers.
Use `caps add --dev <org/repo>@<ref>` and `caps remove --dev <org/repo>` to manage the development group.

`:calcit-version` helps with version checks and provides hints in [CI](https://github.com/calcit-lang/setup-calcit).

To load modules, use `:modules` configuration and the runtime snapshot file `calcit.cirru`:

```cirru.no-check
:entries $ {}
  :default $ {}
    :modules $ [] |memof/calcit.cirru |lilac/
```

Paths defined in `:modules` load from the snapshot directory's `.calcit/modules/`, e.g.
`.calcit/modules/memof/calcit.cirru`. Run `caps` to materialize or refresh that project-local view.

Modules ending with `/` are automatically suffixed with `calcit.cirru`. A module containing only retired `compact.cirru` is rejected with migration guidance.

Inspect and verify the resolved graph with:

```bash
caps tree
caps why calcit-lang/memof
caps status
caps verify
caps verify --toolchain # after yarn install, for JS projects
```

### Development

Local validation commands:

```bash
# run tests in Rust
cargo run --bin calcit -- calcit/test.cirru

# run tests in Node.js
cargo run --bin calcit -- calcit/test.cirru js && yarn try-js

# run snippet
cargo run --bin calcit -- eval 'range 100'

# internal compiler/WASM validation when working on this repository
cargo run --bin calcit -- calcit/test.cirru ir
yarn try-wasm
```

For repository development, the usual validation flow is:

```bash
cargo fmt
cargo clippy -- -D warnings
yarn compile
cargo test
yarn check-all
```

- [Cirru Parser](https://github.com/Cirru/parser.rs) for indentation-based syntax parsing.
- [Cirru EDN](https://github.com/Cirru/cirru-edn.rs) for canonical runtime snapshot parsing (`calcit.cirru`).
- [Ternary Tree](https://github.com/calcit-lang/ternary-tree.rs) for immutable list data structure.

Other tools:

- [Error Viewer](https://github.com/calcit-lang/calcit-error-viewer) for displaying `.calcit/error.cirru`
- [IR Viewer](https://github.com/calcit-lang/calcit-ir-viewer) for rendering `program-ir.cirru`

Some resources:

- Dev Logs https://github.com/calcit-lang/calcit/discussions
- 视频记录 https://space.bilibili.com/14227306/channel/seriesdetail?sid=281171

### License

MIT

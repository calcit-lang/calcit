### Calcit Scripting Language

> Semantically a dialect of ClojureScript. Built with Rust. Compiles to JavaScript ES Modules.

- Home https://calcit-lang.org/
- API Doc https://apis.calcit-lang.org/
- Guidebook https://guide.calcit-lang.org/

[Browse examples](https://github.com/calcit-lang/calcit/tree/main/calcit) or also [try WASM version online](https://github.com/calcit-lang/calcit-wasm-play).

Core design:

- Interpreter runs on Rust, extensible with Rust FFI
- Persistent Data Structure
- Indentation-based Cirru syntax, friendly to plain text editing
- Lisp macros, functional style
- Compiles to JavaScript in ES Modules, JavaScript Interop
- Hot code swapping friendly

Current direction:

- `calcit.cirru` is the primary source snapshot; legacy `compact.cirru` is still compatible
- CLI-first development with `cr` and `caps`, designed to work well with AI agents in terminal workflows
- Better CLI editing and validation for CI, docs lookup, module management, and incremental updates

### Install ![GitHub Release](https://img.shields.io/github/v/release/calcit-lang/calcit)

Build and install with Rust:

```bash
# get Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# get Calcit user-facing tools
cargo install calcit --bin cr --bin caps
```

Installed binaries:

- `cr`, the runtime and JS compiler
- `caps`, for downloading dependencies declared in `deps.cirru`

When installing from source, install the same public tools:

```bash
cargo install --path . --bin cr --bin caps
```

To use Calcit in GitHub Actions, try [setup-cr](https://github.com/calcit-lang/setup-cr).

### Quick Start

Evaluate snippets:

```bash
cr eval 'range 100'

cr eval 'thread-first 100 range (map $ \ * % %)'
```

Run with a runtime snapshot such as `calcit.cirru` (legacy filename: `compact.cirru`):

```bash
cr calcit.cirru # run once (default)
cr compact.cirru # legacy filename still works

cr # by default, it picks `calcit.cirru`, then falls back to `compact.cirru`

cr -w # watch mode (explicit flag required)
```

By default Calcit reads `:init-fn` and `:reload-fn` from `calcit.cirru` configs (falling back to `compact.cirru`). You may also specify functions:

```bash
cr --init-fn='app.main/main!' --reload-fn='app.main/reload!'
```

You may also configure `:entries` in `calcit.cirru`:

```bash
cr --entry server
```

### JavaScript codegen

Calcit compiles to JavaScript with consistent semantics. In browser or Node projects, JavaScript interop is still expected.

```bash
cr js # compile to js, also picks `calcit.cirru` by default
cr js --emit-path=out/ # compile to js and save in `out/`
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
cr docs agents --full   # read the current agent workflow guide
cr query search 'foo'   # locate code by symbol or string
cr edit ...             # structured edits for defs, imports, config, modules
cr js                   # compile once
cr js -w                # watch mode
caps                    # install/update dependencies from deps.cirru
```

Calcit Editor is no longer the recommended path for everyday development. If you still need the older editor workflow, see [Calcit Editor](https://github.com/calcit-lang/editor).

Related examples and workflows:

- [Minimal Calcit](https://github.com/calcit-lang/minimal-calcit/blob/main/README.md)
- [Respo Calcit Workflow](https://github.com/calcit-lang/respo-calcit-workflow)
- [setup-cr](https://github.com/calcit-lang/setup-cr) for GitHub Actions

### Modules

`deps.cirru` declares dependencies that need to download, which correspond to repositories on GitHub. Specify a branch or a tag:

```cirru
{}
  :calcit-version |0.9.11
  :dependencies $ {}
    |calcit-lang/memof |0.0.11
    |calcit-lang/lilac |main
  :dev-dependencies $ {}
    |calcit-lang/calcit-test |0.1.0
```

Run `caps` to resolve the recursive dependency graph and install it. Immutable revisions are stored under
`~/.config/calcit/module-caches/`, while the current project receives links under `.calcit/modules/`.
Different projects can therefore use different revisions without switching a shared checkout. Existing
project module links are the only runtime module source.

Published SemVer tags are preferred. Branch refs remain supported for development, but `caps` warns with
the resolved commit. When a graph requests several SemVer tags for one repository, the highest requested
version is selected and reported.

Root projects install both `:dependencies` and `:dev-dependencies`. Recursive resolution only follows
`:dependencies`, so test and maintenance modules declared by a dependency do not leak into consumers.
Use `caps add --dev <org/repo>@<ref>` and `caps remove --dev <org/repo>` to manage the development group.

`:calcit-version` helps with version checks and provides hints in [CI](https://github.com/calcit-lang/setup-cr).

To load modules, use `:modules` configuration and the runtime snapshot file `calcit.cirru` (legacy: `compact.cirru`):

```cirru
:entries $ {}
  :default $ {}
    :modules $ [] |memof/calcit.cirru |lilac/
```

Paths defined in `:modules` first load from the snapshot directory's `.calcit/modules/`, then fall back to
`~/.config/calcit/modules/`, i.e. `.calcit/modules/memof/calcit.cirru` or the legacy global path.

Modules ending with `/` are automatically suffixed with `calcit.cirru`, and still fall back to `compact.cirru` for compatibility.

Inspect and verify the resolved graph with:

```bash
caps tree
caps why calcit-lang/memof
caps status
caps verify
```

### Development

Local validation commands:

```bash
# run tests in Rust
cargo run --bin cr -- calcit/test.cirru

# run tests in Node.js
cargo run --bin cr -- calcit/test.cirru js && yarn try-js

# run snippet
cargo run --bin cr -- eval 'range 100'

# internal compiler/WASM validation when working on this repository
cargo run --bin cr -- calcit/test.cirru ir
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
- [Cirru EDN](https://github.com/Cirru/cirru-edn.rs) for runtime snapshot file parsing (`calcit.cirru` / legacy `compact.cirru`).
- [Ternary Tree](https://github.com/calcit-lang/ternary-tree.rs) for immutable list data structure.

Other tools:

- [Error Viewer](https://github.com/calcit-lang/calcit-error-viewer) for displaying `.calcit/error.cirru`
- [IR Viewer](https://github.com/calcit-lang/calcit-ir-viewer) for rendering `program-ir.cirru`

Some resources:

- Dev Logs https://github.com/calcit-lang/calcit/discussions
- 视频记录 https://space.bilibili.com/14227306/channel/seriesdetail?sid=281171

### License

MIT

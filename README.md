### Calcit Scripting Language

> Semantically a dialect of ClojureScript. Built with Rust. Compiles to JavaScript ES Modules.

- Home https://calcit-lang.org/
- API Doc https://apis.calcit-lang.org/
- Guidebook https://guide.calcit-lang.org/

[Browse examples](https://github.com/calcit-lang/calcit/tree/main/calcit) or also [try WASM version online](https://github.com/calcit-lang/calcit-wasm-play).

Core design:

- Interpreter runs on Rust, extensible with Rust FFI
- Persistent Data Structure
- Structural Editor(with indentation-based syntax as a fallback)
- Lisp macros, functional style
- Compiles to JavaScript in ES Modules, JavaScript Interop
- Hot code swapping friendly

### Install ![GitHub Release](https://img.shields.io/github/v/release/calcit-lang/calcit)

Build and install with Rust:

```bash
# get Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# get Calcit
cargo install calcit
```

3 binaries are installed:

- `calcit`, the runtime and js compiler
- `caps`, for downloading dependencies declared in `deps.cirru`
- `bundle_calcit`, bundle code if you don't want to use Calcit Editor

To use Calcit in GitHub Actions, try [setup-cr](https://github.com/calcit-lang/setup-cr).

### Usage

Snippets evaling:

```bash
cr eval 'range 100'

cr eval 'thread-first 100 range (map $ \ * % %)'
```

Run with a [compact.cirru](https://github.com/calcit-lang/lilac/blob/main/compact.cirru):

```bash
cr compact.cirru -1 # run only once

cr -1 # by default, it picks `compact.cirru`

cr # watch mode enabled by default
```

By default Calcit reads `:init-fn` and `:reload-fn` inside `compact.cirru` configs. You may also specify functions,

```bash
cr --init-fn='app.main/main!' --reload-fn='app.main/reload!'
```

and even configure `:entries` in `compact.cirru`:

```bash
cr --entry server
```

### JavaScript codegen

It compiles to JavaScript and runs in consistet semantics. However it might require a lot of JavaScript interop.

```bash
cr js # compile to js, also picks `compact.cirru` by default
cr js --emit-path=out/ # compile to js and save in `out/`
```

By default, js code is generated to `js-out/`. You will need Vite or Node to run it, from an entry file:

```js
import { main_$x_, reload_$x_ } from "./js-out/app.main.mjs";
main_$x_(); // which corresponds to `main!` function in calcit
```

### Calcit Editor & Bundler

Install [Calcit Editor](https://github.com/calcit-lang/editor) and run `ct` to launch editor server,
which writes `compact.cirru` and `.compact-inc.cirru` on saving. Try launching example by cloning [Calcit Workflow](https://github.com/calcit-lang/calcit-workflow).

Read more in [Minimal Calcit](https://github.com/calcit-lang/minimal-calcit/blob/main/README.md) to learn how to code Calcit with a plain text editor.

Read more in [Respo Calcit Workflow](https://github.com/calcit-lang/respo-calcit-workflow) to learn to create an MVC webpage with [Respo](http://respo-mvc.org/).

### Modules

`deps.cirru` declares dependencies that need to download, which correspond to repositories on GitHub. Specify a branch or a tag:

```cirru
{}
  :calcit-version |0.9.8
  :dependencies $ {}
    |calcit-lang/memof |0.0.11
    |calcit-lang/lilac |main
```

Run `caps` to download. Sources are downloaded into `~/.config/calcit/modules/`. If a module contains `build.sh`, it will be executed mostly for compiling Rust dylibs.

`:calcit-version` helps in check version, and provides hints in [CI](https://github.com/calcit-lang/setup-cr) environment.

To load modules, use `:modules` configuration and `compact.cirru`(which normally generated from `calcit.cirru`):

```cirru
:configs $ {}
  :modules $ [] |memof/compact.cirru |lilac/
```

Paths defined in `:modules` field are just loaded as files from `~/.config/calcit/modules/`,
i.e. `~/.config/calcit/modules/memof/compact.cirru`.

Modules that ends with `/`s are automatically suffixed `compact.cirru` since it's the default entry.

### Development

I use these commands to run local examples:

```bash
# run tests in Rust
cargo run --bin cr -- calcit/test.cirru -1

# run tests in Node.js
cargo run --bin cr -- calcit/test.cirru -1 js && yarn try-js

# run snippet
cargo run --bin cr -- eval 'range 100'

cr compact.cirru -1 ir # compiles intermediate representation into program-ir.cirru
```

- [Cirru Parser](https://github.com/Cirru/parser.rs) for indentation-based syntax parsing.
- [Cirru EDN](https://github.com/Cirru/cirru-edn.rs) for `compact.cirru` file parsing.
- [Ternary Tree](https://github.com/calcit-lang/ternary-tree.rs) for immutable list data structure.

Other tools:

- [Error Viewer](https://github.com/calcit-lang/calcit-error-viewer) for displaying `.calcit-error.cirru`
- [IR Viewer](https://github.com/calcit-lang/calcit-ir-viewer) for rendering `program-ir.cirru`

Some resources:

- Dev Logs https://github.com/calcit-lang/calcit/discussions
- 视频记录 https://space.bilibili.com/14227306/channel/seriesdetail?sid=281171

### License

MIT

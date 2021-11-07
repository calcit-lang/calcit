### Calcit Scripting Language

> Lisp compiling to JavaScript ES Modules. (Similar to ClojureScript, but in very different syntax.)

- Home http://calcit-lang.org/
- API Doc(heavily influenced by ClojureScript) http://apis.calcit-lang.org/
- Dev Logs https://github.com/calcit-lang/calcit_runner.rs/discussions
- 视频记录 https://space.bilibili.com/14227306/channel/seriesdetail?sid=281171

### Install

Build and install with Rust:

```bash
# get Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# get Calcit
git clone git@github.com:calcit-lang/calcit_runner.rs.git calcit/
cd calcit/
cargo install --path=./
```

For Ubuntu 20.04, try binaries from http://bin.calcit-lang.org/linux/ , which are provided for [CI usages](https://github.com/calcit-lang/respo-calcit-workflow/blob/main/.github/workflows/upload.yaml#L28-L37).

You can also [try it online with WASM](https://github.com/calcit-lang/calcit-wasm-play).

### Usage

Run:

```bash
cr compact.cirru --1 # run only once

cr compact.cirru # watch mode enabled by default

cr compact.cirru --init-fn='app.main/main!' # specifying init-fn
```

Inline evaling:

```bash
cr -e 'range 100'


# multi-lines snippet
cr -e '

println "|a demo"

->
  range 100
  map $ fn (x)
    * x x

'
```

Emitting code:

```bash
cr compact.cirru --emit-js # compile to js
cr compact.cirru --emit-js --emit-path=out/ # compile to js and save in `out/`

cr compact.cirru --emit-ir # compiles intermediate representation into program-ir.cirru
```

### Calcit Editor & Bundler

Install [Calcit Editor](https://github.com/calcit-lang/editor) and run `ct` to launch editor server,
which writes `compact.cirru` and `.compact-inc.cirru` on saving. Try launching example by clong [Calcit Workflow](https://github.com/calcit-lang/calcit-workflow).

Read more in [Minimal Calcit](https://github.com/calcit-lang/minimal-calcit/blob/main/README.md) to learn how to code Calcit with a plain text editor.

Read more in [Respo Calcit Workflow](https://github.com/calcit-lang/respo-calcit-workflow) to learn to create an MVC webpage with [Respo](http://respo-mvc.org/).

### Modules

> No package manager yet, need to manage modules with git tags.

Configurations inside `calcit.cirru` and `compact.cirru`:

```cirru
:configs $ {}
  :modules $ [] |memof/compact.cirru |lilac/
```

Paths defined in `:modules` field are just loaded as files from `~/.config/calcit/modules/`,
i.e. `~/.config/calcit/modules/memof/compact.cirru`.

Modules that ends with `/`s are automatically suffixed `compact.cirru` since it's the default filename.

To load modules in CI environments, make use of `git clone`.

Web Frameworks:

- [Respo](https://github.com/Respo/respo.calcit) - tiny Virtual DOM library
- [Phlox](https://github.com/Phlox-GL/phlox) - wraps PIXI.js in Virtual DOM style
- [Quamolit](https://github.com/Quamolit/quamolit.calcit/) - wraps Three.js in Virtual DOM style
- [Quatrefoil](https://github.com/Quatrefoil-GL/quatrefoil) - Canvas API in virtual DOM style, with ticking rendering

Mini libraries:

- [Lilac](https://github.com/calcit-lang/lilac), data validation tool
- [Memof](https://github.com/calcit-lang/memof), caching tool
- [Recollect](https://github.com/calcit-lang/recollect), diffing tool
- [Calcit Test](https://github.com/calcit-lang/calcit-test), testing tool
- [Bisection Key](https://github.com/calcit-lang/bisection-key), ...
- [Lilac Parser](https://github.com/calcit-lang/lilac-parser), string parsing tool

### Extensions

Rust supports extending with dynamic libraries, found an example in [dylib-workflow](https://github.com/calcit-lang/dylib-workflow). Currently there are some early extensions:

- [Std](https://github.com/calcit-lang/calcit.std) - some collections of util functions
- [WebSocket server binding](https://github.com/calcit-lang/calcit-wss)
- [HTTP client binding](https://github.com/calcit-lang/calcit-fetch)
- [HTTP server binding](https://github.com/calcit-lang/calcit-http)
- [Wasmtime binding](https://github.com/calcit-lang/calcit_wasmtime)
- [Canvas demo](https://github.com/calcit-lang/calcit-paint)

### Development

I use these commands to run local examples:

```bash
# run tests in Rust
cargo run --bin cr -- calcit/snapshots/test.cirru -1

# run tests in Node.js
cargo run --bin cr -- calcit/snapshots/test.cirru --emit-js -1 && yarn try-js

# run snippet
cargo run --bin cr -- -e 'range 100'
```

- [Cirru Parser](https://github.com/Cirru/parser.rs) for indentation-based syntax parsing.
- [Cirru EDN](https://github.com/Cirru/cirru-edn.rs) for `compact.cirru` file parsing.
- [Ternary Tree](https://github.com/calcit-lang/ternary-tree.rs) for immutable list data structure.

Other tools:

- [Error Viewer](https://github.com/calcit-lang/calcit-error-viewer) for displaying `.calcit-error.cirru`
- [IR Viewer](https://github.com/calcit-lang/calcit-ir-viewer) for rendering `program-ir.cirru`

### License

MIT

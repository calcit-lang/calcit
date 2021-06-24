### Calcit Runner

> (Lisp but with indentations.)
> An interpreter for Calcit snapshot file.

- Home http://calcit-lang.org/
- APIs http://apis.calcit-lang.org/

This project provides a runner for `compact.cirru`, written in Rust for low overhead.

APIs implemented in Calcit Runner is mostly learning from Clojure. Major difference arguments order of list functions.

### Installation

For Ubuntu 20.04 users, binaries are available from http://bin.calcit-lang.org/linux/ .
It was mainly provided for [CI usages](https://github.com/calcit-lang/respo-calcit-workflow/blob/master/.github/workflows/upload.yaml#L16-L25).

For other platforms, I'm afraid you have to build from source in Rust with `cargo install --path=./`.

### Calcit snapshot & Bundler

Running [Calcit Editor](https://github.com/Cirru/calcit-editor#compact-output) with `compact=true caclcit-editor` enables compact mode,
which writes `compact.cirru` and `.compact-inc.cirru` instead of Clojure(Script).

A `compact.cirru` file may look like:

```cirru
{} (:package |app)
  :configs $ {} (:init-fn |app.main/main!) (:reload-fn |app.main/reload!)
    :modules $ []
  :files $ {}
    |app.main $ {}
      :ns $ quote
        ns app.main $ :require
      :defs $ {}
        |main! $ quote
          defn main! () (+ 1 2)
        |reload! $ quote
          defn reload! ()
```

But, you probably don't like it. You only to edit code with a simple text editor. So.... there's also command for bundling `compact.cirru` from separated files:

```bash
package.cirru
src/
  app.main.cirru
  app.lib.cirru
```

`package.cirru` should contain fields:

```cirru
{}
  :package |app
  :modules $ []
  :init-fn |app.main/main!
  :reload-fn |app.main/reload!
  :version |0.0.1
```

and files in `src/` are source files of namespace form and definitions. By running:

```bash
bundle_calcit --src ./src --out ./compact.cirru
```

a bundled `compact.cirru` file will be created.

### Usage

Run:

```bash
cr run calcit/compact.cirru

# evaluate

cr compact.cirru --once # run only once
cr compact.cirru # watch mode enabled by default

cr compact.cirru --init-fn='app.main/main!' # specifying init-fn

cr -e="range 100" # eval from CLI

# emit code

cr compact.cirru --emit-js # compile to js
cr compact.cirru --emit-js --emit-path=out/ # compile to js and save in `out/`

cr compact.cirru --emit-ir # compiles intermediate representation into program-ir.json

cr compact.cirru --emit-js --mjs # TODO compile to mjs
```

### Modules

Configurations inside `calcit.cirru` and `compact.cirru`:

```cirru
:configs $ {}
  :modules $ [] |memof/compact.cirru |lilac/
```

Paths defined in `:modules` field are just loaded as files from `~/.config/calcit/modules/`,
i.e. `~/.config/calcit/modules/memof/compact.cirru`.

Modules that ends with `/`s are automatically suffixed `compact.cirru` since it's the default filename.

To load modules in CI environments, make use of `git clone`.

### Development

I use these commands to run local examples:

```bash
cargo run --bin cr calcit/snapshots/test.cirru # a bunch of local tests

cargo run --bin cr calcit/snapshots/test.cirru --emit-js

cargo run --bin cr -- -e 'range 100'

cargo run --bin cr calcit/compact.cirru # this example combined with calcit-editor
```

- [Cirru Parser](https://github.com/Cirru/parser.rs) for indentation-based syntax parsing.
- [Cirru EDN](https://github.com/Cirru/cirru-edn.rs) for `compact.cirru` file parsing.

### Older version

This interpreter was first implemented in [Nim](https://github.com/calcit-lang/calcit-runner.nim) and then switch to Rust. Main change is the order of arguments where operands are now placed at first.

### License

MIT

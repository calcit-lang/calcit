### Calcit Runner

> An interpreter for Calcit snapshot file.

- Home http://calcit-lang.org/
- APIs http://apis.calcit-lang.org/

Running [Calcit Editor](https://github.com/Cirru/calcit-editor#compact-output) with `compact=true caclcit-editor` enables compact mode,
which writes `compact.cirru` and `.compact-inc.cirru` instead of Clojure(Script).
And this project provides a runner for `compact.cirru`, written on Rust for low overhead.

A `compact.cirru` file can be:

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

APIs implemented in Calcit Runner is mostly learning from Clojure. Major difference arguments order of list functions.

### Usage

Run:

```bash
cargo run calcit/compact.cirru

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

For linux users, download pre-built binaries from http://bin.calcit-lang.org/linux/ .

### Bundler

There's also another command for bundling `compact.cirru` from separated files:

```bash
package.cirru
src/
  app.main.cirru
  app.lib.cirru
```

`package.cirru` should contain fields like.

```cirru
{}
  :package |app
  :modules $ []
  :init-fn |app.main/main!
  :reload-fn |app.main/reload!
  :version |0.0.1
```

and files in `src/` are source files of namespace form and definitions. Ny running:

```bash
bundle_calcit --src ./src --out ./compact.cirru
```

a bundled `compact.cirru` file will be included.

### Development

- [Cirru Parser](https://github.com/Cirru/parser.rs) for indentation-based syntax parsing.
- [Cirru EDN](https://github.com/Cirru/cirru-edn.rs) for `compact.cirru` file parsing.

### Modules

```cirru
:configs $ {}
  :modules $ [] |phlox/compact.cirru
```

Calcit Runner use `~/.config/calcit/modules/` as modules directory.
Paths defined in `:modules` field are just loaded as files based on this directory,
which is: `~/.config/calcit/modules/phlox/compact.cirru`.

To load modules in CI environment, create that folder and clone repos manually.

### Older version

This interpreter was first implemented in [Nim](https://github.com/calcit-lang/calcit-runner) and then switch to Rust. Main change is the order of arguments where operands are now placed at first.

### License

MIT

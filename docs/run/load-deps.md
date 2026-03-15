# Load Dependencies

`caps` command is used for downloading dependencies declared in `deps.cirru`. The name "caps" stands for "Calcit Dependencies".

`deps.cirru` declares dependencies, which correspond to repositories on GitHub. Specify a branch or a tag:

```cirru
{}
  :calcit-version |0.9.18
  :dependencies $ {}
    |calcit-lang/memof |0.0.11
    |calcit-lang/lilac |main
```

Run `caps` to download. Sources are downloaded into `~/.config/calcit/modules/`. If a module contains `build.sh`, it will be executed mostly for compiling Rust dylibs.

To load modules, use `:modules` configuration in `calcit.cirru` and `compact.cirru`:

```cirru
:configs $ {}
  :modules $ [] |memof/compact.cirru |lilac/
```

Paths defined in `:modules` field are just loaded as files from `~/.config/calcit/modules/`, i.e. `~/.config/calcit/modules/memof/compact.cirru`.

Modules that ends with `/`s are automatically suffixed `compact.cirru` since it's the default filename.

### Outdated

To check outdated modules, run:

```bash
caps outdated
```

### CLI Options

```
caps --help
Usage: caps [<input>] [-v] [--pull-branch] [--ci] [--local-debug] [<command>] [<args>]

Top-level command.

Positional Arguments:
  input             input file

Options:
  -v, --verbose     verbose mode
  --pull-branch     pull branch in the repo
  --ci              CI mode loads shallow repo via HTTPS
  --local-debug     debug mode, clone to test-modules/
  --help, help      display usage information

Commands:
  outdated          show outdated versions
  download          download named packages with org/repo@branch
```

- "pull branch" to fetch update if only branch name is specified like `main`.
- "ci" does not support `git@` protocol, only `https://` protocol.

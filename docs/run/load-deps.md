---
title: "Load Dependencies"
scope: "core"
kind: "reference"
category: "run"
aliases:
  - "load dependencies"
  - "deps.cirru"
  - "caps"
---
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

To load modules, use `:modules` configuration in `calcit.cirru` (legacy filename: `compact.cirru`):

```cirru.edn
{}
  :entries $ {}
    :default $ {}
      :modules $ [] |memof/calcit.cirru |lilac/
```

Paths defined in `:modules` field are just loaded as files from `~/.config/calcit/modules/`, i.e. `~/.config/calcit/modules/memof/calcit.cirru`.

Modules that ends with `/`s are automatically suffixed `calcit.cirru`, and still fall back to `compact.cirru` for compatibility.

### Outdated

To check outdated modules, run:

```bash
caps outdated
```

To update `deps.cirru` directly without confirmation:

```bash
caps upgrade --all
```

### Module status and local changes

Installed modules are Git working trees. Check that every installed module is at the
version declared by `deps.cirru`, and report local working-tree changes, with:

```bash
caps status
```

The regular `caps` command performs the same local-change check before syncing
dependencies. If a module has local modifications, it prints a warning so that the
changes are not mistaken for the version from the remote repository.

To discard tracked local changes and return each installed dependency to its current
commit, run:

```bash
caps reset
```

This uses `git reset --hard HEAD`; review and back up any work you need before running
it. Untracked files are reported by `caps status` but are not deleted by `caps reset`.

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
  status            check installed module versions and local modifications
  reset             discard tracked local modifications in installed modules
```

- "pull branch" to fetch update if only branch name is specified like `main`.
- "ci" does not support `git@` protocol, only `https://` protocol.

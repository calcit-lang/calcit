cargo install calcit

# Installation

To install Calcit, you first need to install Rust. Then, you can install Calcit using Rust's package manager:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

After installing Rust, install Calcit with:

```bash
cargo install calcit
```

Once installed, Calcit is available as a command-line tool. You can test it with:

```bash
cr eval "echo |done"
```

### Binaries

Several binaries are included:

- `cr`: the main command-line tool for running Calcit programs
- `bundle_calcit`: bundles Calcit code into a `compact.cirru` file
- `caps`: downloads Calcit packages
- `cr-mcp`: provides a Model Context Protocol (MCP) server for Calcit compact files
- `cr-sync`: syncs changes from `compact.cirru` back to `calcit.cirru`

Another important command is `ct`, which is the "Calcit Editor" and is available in a separate repository.

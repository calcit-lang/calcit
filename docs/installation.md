---
title: "Installation"
scope: "core"
kind: "hub"
category: "installation"
aliases:
  - "install calcit"
  - "cargo install"
  - "setup"
---
cargo install calcit

# Installation

To install Calcit, you first need to install Rust. Then, you can install Calcit using Rust's package manager:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

After installing Rust, install the runtime and package manager from their independent releases:

```bash
cargo install calcit --bin calcit
cargo install calcit-caps
```

Once installed, Calcit is available as a command-line tool. You can test it with:

```bash
calcit eval "echo |done"
```

### Binaries

The two normal user-facing commands are independently versioned:

- `calcit`: the main command-line tool for running and compiling Calcit programs, from the `calcit` crate
- `caps`: downloads and verifies Calcit packages, from the [`calcit-caps`](https://github.com/calcit-lang/caps) crate

Another important command is `ct`, which is the "Calcit Editor" and is available in a separate repository.

## Native extensions and host capabilities

- [Core, library, and host capability boundary](installation/host-capability-boundary.md)
- [Rust bindings](installation/ffi-bindings.md)
- [FFI Interface IR](installation/ffi-interface-ir.md)
- [WASM Component 边界](installation/wasm-component-boundary.md)
- [有界类型化 WASI HTTP](installation/wasi-http.md)（可复制示例见 [`examples/wasi-http-client/`](../examples/wasi-http-client/)）
- [FFI async protocol](installation/ffi-async-protocol.md)
- [FFI resource protocol](installation/ffi-resource-protocol.md)
- [FFI upgrade guide](installation/ffi-upgrade-guide.md)

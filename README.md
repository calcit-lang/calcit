### Calcit Runner

> ... in Rust.

Experimental rewrite based on [Nim version](https://github.com/calcit-lang/calcit-runner) trying to make use of ADT.

### Usages

Build a `calcit_runner` command:

```bash
Calcit Runner

USAGE:
    calcit_runner [FLAGS] [OPTIONS] [input]

FLAGS:
        --emit-ir    emit JSON representation of program to program-ir.json
        --emit-js    emit js rather than interpreting
    -h, --help       Prints help information
    -1, --once       disable watching mode
    -V, --version    Prints version information

OPTIONS:
    -e, --eval <eval>    eval a snippet

ARGS:
    <input>    entry file path, defaults to compact.cirru [default: compact.cirru]
```

### License

MIT

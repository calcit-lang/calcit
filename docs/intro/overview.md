# Overview

- Immutable Data

Values and states are represented in different data structures, which is the semantics from functional programming. Internally it's [im](https://crates.io/crates/im) in Rust and a custom [finger tree](https://github.com/calcit-lang/ternary-tree.ts) in JavaScript.

- Lisp(Code is Data)

Calcit-js was designed based on experiences from ClojureScript, with a bunch of builtin macros. It offers similar experiences to ClojureScript. So Calcit offers much power via macros, while keeping its core simple.

- Indentations

With the `cr` command, Calcit code can be written as an indentation-based language directly. So you don't have to match parentheses like in Clojure. It also means now you need to handle indentations very carefully.

- Hot code swapping

Calcit was built with hot swapping in mind. Combined with [calcit-editor](https://github.com/calcit-lang/editor), it watches code changes by default, and re-runs program on updates. For calcit-js, it works with Vite and Webpack to reload, learning from Elm, ClojureScript and React.

- ES Modules Syntax

To leverage the power of modern browsers with help of Vite, we need another ClojureScript that emits `import`/`export` for Vite. Calcit-js does this! And this page is built with Calcit-js as well, open Console to find out more.

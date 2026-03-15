# Features from Clojure

Calcit is mostly a ClojureScript dialect. So it should also be considered a Clojure dialect.

There are some significant features Calcit is learning from Clojure,

- Runtime persistent data by default, you can only simulate states with `Ref`s.
- Namespaces
- Hygienic macros(although less powerful)
- Higher order functions
- Keywords, although Calcit changed the name to "tag" since `0.7`
- Compiles to JavaScript, interops
- Hot code swapping while code modified, and trigger an `on-reload` function
- HUD for JavaScript errors

Also there are some differences:

| Feature           | Calcit                                           | Clojure                                      |
| ----------------- | ------------------------------------------------ | -------------------------------------------- |
| Host Language     | Rust, and use `dylib`s for extending             | Java/Clojure, import Mavan packages          |
| Syntax            | Indentations / Syntax Tree Editor                | Parentheses                                  |
| Persistent data   | unbalanced 2-3 Tree, with tricks from FingerTree | HAMT / RRB-tree                              |
| Package manager   | `git clone` to a folder                          | Clojars                                      |
| bundle js modules | ES Modules, with ESBuild/Vite                    | Google Closure Compiler / Webpack            |
| operand order     | at first                                         | at last                                      |
| Polymorphism      | at runtime, slow `.map ([] 1 2 3) f`             | at compile time, also supports multi-arities |
| REPL              | only at command line: `cr eval "+ 1 2"`          | a real REPL                                  |
| `[]` syntax       | `[]` is a built-in function                      | builtin syntax                               |
| `{}` syntax       | `{} (:a b)` is macro, expands to `&{} :a :b`     | builtin syntax                               |

also Calcit is a one-person language, it has too few features compared to Clojure.

Calcit shares many paradiams I learnt while using ClojureScript. But meanwhile it's designed to be more friendly with ES Modules ecosystem.

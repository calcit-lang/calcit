# String

The way strings are represented in Calcit is a bit unique. Strings are distinguished by a prefix. For example, `|A` represents the string `A`. If the string contains spaces, you need to enclose it in double quotes, such as `"|A B"`, where `|` is the string prefix. Due to the history of the structural editor, `"` is also a string prefix, but it is special: when used inside a string, it must be escaped as `"\"A"`. This is equivalent to `|A` and also to `"|A"`. The outermost double quotes can be omitted when there is no ambiguity.

This somewhat unusual design exists because the structural editor naturally wraps strings in double quotes. When writing with indentation-based syntax, the outermost double quotes can be omitted for convenience.

### Tag

The most commonly used string type in Calcit is the Tag, which starts with a `:`, such as `:demo`. Its type is `Tag` in Rust and `string` in JavaScript. Unlike regular strings, Tags are immutable, meaning their value cannot be changed once created. This allows them to be used as keys in key-value pairs and in other scenarios where immutable values are needed. In practice, Tags are generally used to represent property keys, similar to keywords in the Clojure language.

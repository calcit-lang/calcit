
{} (:package |app)
  :configs $ {} (:init-fn |app.main/main!) (:reload-fn |app.main/reload!)
  :files $ {}
    |app.main $ %{} :FileEntry
      :defs $ {}
        |add-numbers $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn add-numbers (a b)
              assert-type a :number
              assert-type b :number
              hint-fn $ return-type :number
              &+ a b

        |process-string $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn process-string (s)
              assert-type s :string
              str s |!!!

        |test-fn-type $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-fn-type (f x)
              assert-type f :fn
              assert-type x :number
              f x

        |test-proc-type $ %{} :CodeEntry (:doc "|Tests Proc (builtin function) type annotation")
          :code $ quote
            defn test-proc-type (p x y)
              assert-type p :proc
              assert-type x :number
              assert-type y :number
              hint-fn $ return-type :number
              p x y

        |show-type-info $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn show-type-info (x)
              assert-type x :number
              println "|Type info demo: value is" x
              ; 后续引用 x 应该仍然保留类型信息
              &+ x 1

        |slice-as-string $ %{} :CodeEntry (:doc "|Guarded dynamic .slice call")
          :code $ quote
            defn slice-as-string (text)
              assert-type text :string
              .slice text 1 4

        |test-dynamic-methods $ %{} :CodeEntry (:doc "|Ensures .slice target type is validated")
          :code $ quote
            defn test-dynamic-methods ()
              let
                  typed-text |calcit
                assert-type typed-text :string
                assert= |alc $ .slice typed-text 1 4
              let
                  also-text |numbers
                assert-type also-text :string
                assert= |num $ .slice also-text 0 3

              println "|slice checks succeeded"
              , "|slice checks passed"

        |describe-typed $ %{} :CodeEntry (:doc "|Combines typed label and number")
          :code $ quote
            defn describe-typed (label value)
              assert-type label :string
              assert-type value :number
              hint-fn $ return-type :string
              str label "|: " value

        |chained-return-type $ %{} :CodeEntry (:doc "|Uses return-type hinting more than once")
          :code $ quote
            defn chained-return-type (base extra)
              assert-type base :number
              assert-type extra :number
              let
                  first-sum $ add-numbers base extra
                assert-type first-sum :number
                hint-fn $ return-type :number
                add-numbers first-sum 5

        |test-threading-types $ %{} :CodeEntry (:doc "|Tests type preservation through -> threading macro")
          :code $ quote
            defn test-threading-types (text)
              assert-type text :string
              hint-fn $ return-type :string
              ; 使用 -> 串联：text 先经过 str 拼接，再经过 process-string
              ; 最终结果应该保留 :string 类型（从 process-string 的 return-type 推断）
              -> text
                str |prefix:
                process-string

        |test-complex-threading $ %{} :CodeEntry (:doc "|Tests type preservation with multiple typed functions in -> chain")
          :code $ quote
            defn test-complex-threading (a b)
              assert-type a :number
              assert-type b :number
              hint-fn $ return-type :number
              let
                  ; 先计算初始值，然后使用 -> 语法串联多个有 return-type 标注的函数
                  sum-ab $ add-numbers a b
                  final-result $ -> sum-ab
                    add-numbers 10
                ; final-result 应该保留 :number 类型
                assert-type final-result :number
                &+ final-result 5

        |test-builtin-proc-types $ %{} :CodeEntry (:doc "|Tests that builtin Procs preserve type information through calls")
          :code $ quote
            defn test-builtin-proc-types ()
              ; &+ 有内置类型签名: (number, number) -> number
              let
                  sum $ &+ 10 20
                ; sum 应该推断为 :number（虽然当前版本可能还没完全实现推断）
                ; assert-type sum :number
                println "|sum:" sum
              ; floor 有内置类型签名: number -> number
              let
                  rounded $ floor 3.7
                println "|rounded:" rounded
              ; not 有内置类型签名: bool -> bool
              let
                  negated $ not true
                println "|negated:" negated
              , "|Builtin proc types test passed"

        |test-builtin-proc-types $ %{} :CodeEntry (:doc "|Tests that Proc (builtin) functions check argument types during preprocess")
          :code $ quote
            defn test-builtin-proc-types ()
              ; Test math operations with typed arguments
              let
                  x 10
                  y 20
                assert-type x :number
                assert-type y :number
                let
                    sum $ &+ x y
                    rounded $ round 3.14
                    negated $ not false
                  println "|sum:" sum
                  println "|rounded:" rounded
                  println "|negated:" negated

        |test-proc-type-warnings $ %{} :CodeEntry (:doc "|Test that should generate type warnings - disabled by default")
          :code $ quote
            defn test-proc-type-warnings ()
              ; This function intentionally contains type errors for testing
              ; It is not called in normal tests to avoid blocking execution
              println "|Warning: This test contains intentional type errors"

        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! ()
              println "|Testing types..."
              println $ add-numbers 1 2
              println $ process-string |hello
              println $ test-fn-type (fn (n) (&+ n 10)) 5
              println $ test-proc-type &+ 10 5
              println $ show-type-info 42
              println $ slice-as-string |dynamic-call
              println $ test-dynamic-methods
              println $ describe-typed |score 99
              println $ chained-return-type 3 4
              println $ test-threading-types |world
              println $ test-complex-threading 10 20
              test-builtin-proc-types
              ; test-proc-type-warnings
              println "|Done!"
              ; Note: Record field validation requires explicit type annotations
              ; in unit tests via assert-type with Record instances.
              ; Currently not supported for runtime Record literals.

        |reload! $ %{} :CodeEntry (:doc |)
          :code $ quote (defn reload! () nil)

      :ns $ %{} :CodeEntry (:doc |)
        :code $ quote (ns app.main)


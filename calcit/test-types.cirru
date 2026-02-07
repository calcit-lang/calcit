
{} (:package |test-types)
  :configs $ {} (:init-fn |test-types.main/main!) (:reload-fn |test-types.main/reload!)
  :files $ {}
    |test-types.main $ %{} :FileEntry
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
              assert-type p :fn
              assert-type x :number
              assert-type y :number
              hint-fn $ return-type :number
              p x y

        |typed-only $ %{} :CodeEntry (:doc "|Used to verify arg type hints are collected")
          :code $ quote
            defn typed-only (a)
              assert-type a :number
              &+ 1 0

        |test-arg-type-hints $ %{} :CodeEntry (:doc "|Trigger type warning for non-variadic arg hints")
          :code $ quote
            defn test-arg-type-hints ()
              typed-only |oops
              println "|arg type hints check executed"

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

        |test-list-methods $ %{} :CodeEntry (:doc "|Tests method calls on typed list objects")
          :code $ quote
            defn test-list-methods ()
              ; Create a list and annotate its type
              let
                  xs $ [] 1 2 3 4 5
                assert-type xs :list
                ; Call valid list methods
                let
                    first-item $ .first xs
                    second-item $ .nth xs 1
                    rest-items $ .rest xs
                    list-len $ .count xs
                  println "|first:" first-item
                  println "|second:" second-item
                  println "|rest:" rest-items
                  println "|count:" list-len
                  assert= 1 first-item
                  assert= 2 second-item
                  assert= 5 list-len
              , "|List method checks passed"

        |test-string-methods $ %{} :CodeEntry (:doc "|Tests method calls on typed string objects")
          :code $ quote
            defn test-string-methods ()
              let
                  text |hello-world
                assert-type text :string
                ; Call valid string methods
                let
                    sliced $ .slice text 0 5
                    text-len $ .count text
                    first-char $ .first text
                    starts $ .starts-with? text |hello
                    splitted $ .split text |-
                  println "|sliced:" sliced
                  println "|length:" text-len
                  println "|first-char:" first-char
                  println "|starts with hello:" starts
                  println "|split:" splitted
                  assert= |hello sliced
                  assert= 11 text-len
                  assert= |h first-char
                  assert= true starts
              , "|String method checks passed"

        |test-record-methods $ %{} :CodeEntry (:doc "|Tests method calls on Record instances with class")
          :code $ quote
            defn test-record-methods ()
              ; 使用 new-impl-record 创建带 class 的 Record
              let
                  Person $ new-impl-record :Person
                    {} $ :name
                      :get $ fn (self) (:name self)
                    {} $ :age
                      :get $ fn (self) (:age self)
                    {} $ :greet
                      :method $ fn (self) (str "|Hello, I'm " $ :name self)
                ; 创建 Person 实例
                let
                  alice $ impl-traits (:: :name |Alice :age 30) Person
                  ; 调用方法
                  let
                      greeting $ .greet alice
                    println "|greeting:" greeting
                    assert= "|Hello, I'm Alice" greeting
              , "|Record method checks passed"

        |test-method-type-errors $ %{} :CodeEntry (:doc "|Tests that invalid method calls are caught in preprocess")
          :code $ quote
            defn test-method-type-errors ()
              ; ⚠️ 这些代码故意包含错误，用于验证 preprocess 阶段的类型检查
              ; 当启用时，会在编译阶段就报错，而不是运行时

              ; 测试 1: list 对象调用不存在的方法
              ; let
              ;     xs $ [] 1 2 3
              ;   assert-type xs :list
              ;   .invalid-method xs

              ; 测试 2: string 对象调用不存在的方法
              ; let
              ;     text |hello


              ;   assert-type text :string
              ;   .nonexistent text

              ; 测试 3: map 对象调用不存在的方法
              ; let
              ;     m $ {} (:a 1)
              ;   assert-type m :map
              ;   .invalid-map-method m

              println "|Method type error tests are commented out"
              println "|Uncomment them to see preprocess-time validation"
              , "|Tests disabled to allow compilation"

        |test-preprocess-method-validation $ %{} :CodeEntry (:doc "|Demonstrates that valid method calls pass preprocess validation")
          :code $ quote
            defn test-preprocess-method-validation ()
              ; 所有这些方法调用都是合法的，应该通过 preprocess 检查
              let
                  xs $ [] 1 2 3 4 5
                assert-type xs :list
                let
                    first-item $ .first xs
                    count-val $ .count xs
                    reversed $ .reverse xs
                  println "|✓ List methods validated at preprocess"

              let
                  text |hello-world
                assert-type text :string
                let
                    sliced $ .slice text 0 5
                    len $ .count text
                    trimmed $ .trim text
                  println "|✓ String methods validated at preprocess"

              let
                  m $ {} (:a 1) (:b 2)
                assert-type m :map
                let
                    val $ .get m :a
                    keys-list $ .keys m
                    size $ .count m
                  println "|✓ Map methods validated at preprocess"

              , "|All valid method calls passed preprocess validation"

        |test-typed-method-access $ %{} :CodeEntry (:doc "|Demonstrates type-safe method access patterns")
          :code $ quote
            defn test-typed-method-access ()
              ; 当对象有类型标注时，方法调用会检查该类型支持的方法
              let
                  typed-list $ [] 1 2 3 4 5
                assert-type typed-list :list
                ; :list 类型对应 calcit.core/&core-list-methods 提供的方法
                let
                    first-elem $ .first typed-list
                    list-size $ .count typed-list
                    rest-elems $ .rest typed-list
                  println "|Typed list access - first:" first-elem
                  println "|Typed list access - count:" list-size
                  assert= 1 first-elem
                  assert= 5 list-size
              ; 字符串也有类型相关的方法
              let
                  typed-str |test-string
                assert-type typed-str :string
                let
                    str-len $ .count typed-str
                    str-first $ .first typed-str
                  println "|Typed string access - count:" str-len
                  println "|Typed string access - first:" str-first
                  assert= 11 str-len
                  assert= |t str-first
              , "|Typed method access checks passed"

        |Person $ %{} :CodeEntry (:doc "|Struct definition for type checks")
          :code $ quote
            defstruct Person
              :name :string
              :age nil

        |StructImpl $ %{} :CodeEntry (:doc "|Trait impl for struct metadata")
          :code $ quote
            defrecord! StructImpl
              :dummy nil

        |Result $ %{} :CodeEntry (:doc "|Enum prototype for type checks")
          :code $ quote
            defenum Result
              :ok :number
              :err :string

        |ResultImpl $ %{} :CodeEntry (:doc "|Trait impl for enum tuple tests")
          :code $ quote
            defrecord! ResultImpl
              :describe $ fn (self)
                tag-match self
                  (:ok value) (str "|ok " value)
                  (:err msg) (str "|err " msg)

        |EnumImpl $ %{} :CodeEntry (:doc "|Trait impl for enum metadata")
          :code $ quote
            defrecord! EnumImpl
              :dummy nil

        |test-defstruct-defenum $ %{} :CodeEntry (:doc "|Smoke test for defstruct/defenum and %:: tuples")
          :code $ quote
            defn test-defstruct-defenum ()
              assert= :struct $ type-of Person
              assert= :enum $ type-of Result
              assert= :struct $ type-of $ impl-traits Person StructImpl
              let
                  enum-with-impls $ impl-traits Result EnumImpl
                  ok $ impl-traits (%:: enum-with-impls :ok 1) ResultImpl
                assert= :enum $ type-of enum-with-impls
                assert= ResultImpl $ &list:first $ &tuple:impls ok
                assert= enum-with-impls $ &tuple:enum ok
                assert= "|(%:: :ok 1 (:impls ResultImpl) (:enum Result))" $ str ok
              , "|defstruct/defenum checks passed"

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
              test-arg-type-hints
              test-builtin-proc-types
              println "|--- Testing typed method calls ---"
              println $ test-list-methods
              println $ test-string-methods
              ; println $ test-record-methods
              println "|--- Testing typed method access patterns ---"
              println $ test-typed-method-access
              println "|--- Testing preprocess method validation ---"
              println $ test-preprocess-method-validation
              println $ test-defstruct-defenum
              ; test-method-type-errors ; Disabled - contains intentional errors
              ; test-proc-type-warnings
              println "|Done!"
              ; Note: Record field validation requires explicit type annotations
              ; in unit tests via assert-type with Record instances.
              ; Currently not supported for runtime Record literals.

        |reload! $ %{} :CodeEntry (:doc |)
          :code $ quote (defn reload! () nil)

      :ns $ %{} :CodeEntry (:doc |)
        :code $ quote (ns test-types.main)


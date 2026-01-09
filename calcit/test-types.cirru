
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
              let
                  summary $ str label "| -> " value
                assert-type summary :string
                summary

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

        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! ()
              println "|Testing types..."
              println $ add-numbers 1 2
              println $ process-string |hello
              println $ test-fn-type (fn (n) (&+ n 10)) 5
              println $ show-type-info 42
              println $ slice-as-string |dynamic-call
              println $ test-dynamic-methods
              println $ describe-typed |score 99
              println $ chained-return-type 3 4
              println "|Done!"
              ; Note: Record field validation requires explicit type annotations
              ; in unit tests via assert-type with Record instances.
              ; Currently not supported for runtime Record literals.

        |reload! $ %{} :CodeEntry (:doc |)
          :code $ quote (defn reload! () nil)

      :ns $ %{} :CodeEntry (:doc |)
        :code $ quote (ns app.main)



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

        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! ()
              println "|Testing types..."
              println $ add-numbers 1 2
              println $ process-string |hello
              println $ test-fn-type (fn (n) (&+ n 10)) 5
              println $ show-type-info 42
              println "|Done!"
              ; Note: Record field validation requires explicit type annotations
              ; in unit tests via assert-type with Record instances.
              ; Currently not supported for runtime Record literals.

        |reload! $ %{} :CodeEntry (:doc |)
          :code $ quote (defn reload! () nil)

      :ns $ %{} :CodeEntry (:doc |)
        :code $ quote (ns app.main)



{} (:package |app)
  :configs $ {} (:init-fn |app.main/main!) (:reload-fn |app.main/reload!)
  :files $ {}
    |app.main $ %{} :FileEntry
      :defs $ {}
        |test-type-mismatch $ %{} :CodeEntry (:doc "|Demonstrates Proc type checking - intentional type error")
          :code $ quote
            defn test-type-mismatch ()
              ; This should generate a warning: passing string to numeric operation
              let
                  text |hello
                  num 42
                assert-type text :string
                assert-type num :number
                ; Error: &+ expects two numbers, but we're passing a string
                println "|Testing type mismatch..."
                &+ text 10

        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! ()
              println "|=== Proc Type Warning Demo ==="
              println "|This file demonstrates type checking for Proc (builtin) functions"
              println "|Expected warning: Proc &+ arg 1 expects type :number, but got :string"
              println "|"
              test-type-mismatch
              println "|Done!"

        |reload! $ %{} :CodeEntry (:doc |)
          :code $ quote (defn reload! () nil)

      :ns $ %{} :CodeEntry (:doc |)
        :code $ quote (ns app.main)


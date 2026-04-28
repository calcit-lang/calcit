
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |app)
  :configs $ {} (:init-fn |app.main/main!) (:reload-fn |app.main/reload!) (:version |0.0.0)
    :modules $ []
  :entries $ {}
  :files $ {}
    |app.main $ %{} :FileEntry
      :defs $ {}
        |main! $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn main! () (println "|=== Proc Type Warning Demo ===") (println "|This file demonstrates type checking for Proc (builtin) functions") (println "|Expected warning: Proc &+ arg 1 expects type :number, but got :string") (println |) (test-type-mismatch) (println |Done!)
          :examples $ []
        |reload! $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn reload! () nil
          :examples $ []
        |test-type-mismatch $ %{} :CodeEntry (:doc "|Demonstrates Proc type checking - intentional type error") (:schema nil)
          :code $ quote
            defn test-type-mismatch () (; This should generate a warning: passing string to numeric operation)
              let
                  text |hello
                  num 42
                assert-type text :string
                assert-type num :number
                ; Error: &+ expects two numbers, but we're passing a string
                println "|Testing type mismatch..."
                &+ text 10
          :examples $ []
      :ns $ %{} :NsEntry (:doc |)
        :code $ quote (ns app.main)

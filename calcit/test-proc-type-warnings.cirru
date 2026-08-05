
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |app) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/reload!)
      :modules $ []
      :type-slots $ {}
  :files $ {}
    |app.main $ %{} :FileEntry
      :defs $ {}
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! () (println "|=== Proc Type Warning Demo ===") (println "|This file demonstrates type checking for Proc (builtin) functions") (println "|Expected warning: Proc &+ arg 1 expects type :number, but got :string") (println |) (test-type-mismatch) (println |Done!)
          :examples $ []
          :schema $ :: 'Dynamic
        |reload! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn reload! () $ ;nil
          :examples $ []
          :schema $ :: 'Dynamic
        |test-type-mismatch $ %{} :CodeEntry (:doc "|Demonstrates Proc type checking - intentional type error")
          :code $ quote
            defn test-type-mismatch () (; This should generate a warning: passing string to numeric operation)
              let
                  text |hello
                  num 42
                assert-type text 'String
                assert-type num 'Number
                ; Error: &+ expects two numbers, but we're passing a string
                println "|Testing type mismatch..."
                &+ text 10
          :examples $ []
          :schema $ :: 'Dynamic
      :ns $ %{} :NsEntry (:doc |)
        :code $ quote (ns app.main)

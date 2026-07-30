
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |test-invalid-tag) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:description |) (:init-fn |test-invalid-tag.main/main!) (:mode :native) (:reload-fn |test-invalid-tag.main/reload!)
      :modules $ []
      :type-slots $ {}
  :files $ {}
    |test-invalid-tag.main $ %{} :FileEntry
      :defs $ {}
        |Result $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defenum Result (:err :string) (:ok)
          :examples $ []
        |ResultImpl $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defimpl ResultImpl ResultTrait $ .dummy nil
          :examples $ []
        |ResultTrait $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            deftrait ResultTrait $ .dummy :fn
          :examples $ []
        |main! $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defn main! () (println "|Testing %:: call") (println |Result: Result)
              println |ResultImpl: ResultImpl (; Direct call to %:: to see if function is invoked) (println "|Calling %:: ...")
              let
                  result $ %:: Result :invalid
                println "|Should not reach here:" result
          :examples $ []
        |reload! $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defn reload! () nil
          :examples $ []
      :ns $ %{} :NsEntry (:doc |)
        :code $ quote (ns test-invalid-tag.main)

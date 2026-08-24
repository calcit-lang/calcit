
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --full` first. Manual edits must follow format and schema conventions, then run `calcit edit format`.") (:package |test-effects-graph)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'test-effects-graph.main/main!) (:mode :native) (:reload-fn 'test-effects-graph.main/reload!)
      :feature-policy $ {}
      :modules $ [] |./test-method-errors.cirru
      :type-slots $ {}
  :files $ {}
    |test-effects-graph.main $ %{} 'FileEntry
      :defs $ {}
        |io-helper $ %{} 'CodeEntry (:doc "|reads a file path")
          :code $ quote
            defn io-helper (path) (read-file path)
          :examples $ []
          :schema $ :: 'Dynamic
        |main! $ %{} 'CodeEntry (:doc "|entry with io and state effects")
          :code $ quote
            defn main! () $ trigger-type-error
          :examples $ []
          :schema $ :: 'Dynamic
        |reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn reload! () $ :: 'Unit
          :examples $ []
          :schema $ :: 'Dynamic
        |state-helper $ %{} 'CodeEntry (:doc "|defines and mutates an atom")
          :code $ quote
            defn state-helper () (defatom *counter 0) (reset! *counter 1) (swap! *counter inc)
          :examples $ []
          :schema $ :: 'Dynamic
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote
          ns test-effects-graph.main $ :require
            test-method-errors.main :refer $ trigger-type-error

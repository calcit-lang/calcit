
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |test-effects-graph) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:description |) (:init-fn |test-effects-graph.main/main!) (:mode :native) (:reload-fn |test-effects-graph.main/reload!)
      :modules $ []
      :type-slots $ {}
  :files $ {}
    |test-effects-graph.main $ %{} :FileEntry
      :defs $ {}
        |io-helper $ %{} :CodeEntry (:doc "|reads a file path") (:schema :dynamic)
          :code $ quote
            defn io-helper (path) (read-file path)
          :examples $ []
        |main! $ %{} :CodeEntry (:doc "|entry with io and state effects") (:schema :dynamic)
          :code $ quote
            defn main! () (println "|effects-graph smoke") (state-helper) (io-helper |README.md)
          :examples $ []
        |reload! $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defn reload! () $ :: :unit
          :examples $ []
        |state-helper $ %{} :CodeEntry (:doc "|defines and mutates an atom") (:schema :dynamic)
          :code $ quote
            defn state-helper () (defatom *counter 0) (reset! *counter 1) (swap! *counter inc)
          :examples $ []
      :ns $ %{} :NsEntry (:doc |)
        :code $ quote
          ns test-effects-graph.main $

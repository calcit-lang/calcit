
{} (:about "|Effects graph analyzer smoke test") (:package |test-effects-graph)
  :configs $ {} (:init-fn |test-effects-graph.main/main!) (:reload-fn |test-effects-graph.main/reload!) (:version |0.0.0)
    :modules $ []
  :entries $ {}
  :files $ {}
    |test-effects-graph.main $ %{} :FileEntry
      :defs $ {}
        |io-helper $ %{} :CodeEntry (:doc "|reads a file path") (:schema nil)
          :code $ quote
            defn io-helper (path) (read-file path)
          :examples $ []
        |main! $ %{} :CodeEntry (:doc "|entry with io and state effects") (:schema nil)
          :code $ quote
            defn main! ()
              println "|effects-graph smoke"
              state-helper
              io-helper |README.md
          :examples $ []
        |reload! $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn reload! () $ :: :unit
          :examples $ []
        |state-helper $ %{} :CodeEntry (:doc "|defines and mutates an atom") (:schema nil)
          :code $ quote
            defn state-helper ()
              defatom *counter 0
              reset! *counter 1
              swap! *counter inc
          :examples $ []
      :ns $ %{} :NsEntry (:doc |)
        :code $ quote
          ns test-effects-graph.main $


{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |test-effects-graph
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'test-effects-graph.main/main!) (:mode :native) (:reload-fn 'test-effects-graph.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'test-effects-graph.main
    %{} 'FileEntry
      :defs $ {}
        '*store $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defref *store 0
          :examples $ []
        'call-through $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn call-through (f) (f 1)
          :examples $ []
        'cancel-helper $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn cancel-helper (task) (.cancel task)
          :examples $ []
        'io-helper $ %{} 'CodeEntry (:doc "|reads a file path")
          :code $ quote $ defn io-helper (path) (read-file path)
          :examples $ []
          :schema $ :: 'Dynamic
        'js-helper $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn js-helper () (js/console.log |hi)
          :examples $ []
        'load-config $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn load-config ()
            {} $ :port 8080
          :examples $ []
        'main! $ %{} 'CodeEntry (:doc "|entry with io and state effects")
          :code $ quote $ defn main! () (println "|effects-graph smoke") (state-helper) (io-helper |README.md) (setup!) (watch-helper) (cancel-helper nil) (write-helper nil) (call-through inc) (js-helper) (missing-helper)
          :examples $ []
          :schema $ :: 'Dynamic
        'missing-helper $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn missing-helper () (missing.ns/thing 1)
          :examples $ []
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () (:: 'Unit)
          :examples $ []
          :schema $ :: 'Dynamic
        'setup! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn setup! () (load-config)
          :examples $ []
        'state-helper $ %{} 'CodeEntry (:doc "|defines and mutates an atom")
          :code $ quote $ defn state-helper () (defatom *counter 0) (reset! *counter 1) (swap! *counter inc)
          :examples $ []
          :schema $ :: 'Dynamic
        'watch-helper $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn watch-helper () (remove-watch *store :log)
          :examples $ []
        'write-helper $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn write-helper (path) (.write-text! path |done)
          :examples $ []
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns test-effects-graph.main

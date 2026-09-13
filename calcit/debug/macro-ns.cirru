
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |macro-ns
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'macro-ns.main/main!) (:mode :native) (:reload-fn 'macro-ns.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'macro-ns.lib $ %{} 'FileEntry
      :defs $ {}
        'expand-1 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defmacro expand-1 (n) (println "|local data" v)
            quasiquote $ println ~n ~v
          :examples $ []
          :schema $ :: 'Macro $ {}
            :capabilities $ #{} :log
            :expansion $ :: 'Expr 'Dynamic
            :required $ [] $ :: 'Expr 'Dynamic
        'v $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def v 100
          :examples $ []
          :schema $ :: 'Dynamic
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns macro-ns.lib
          :require $ [] util.core :refer $ [] log-title inside-eval:
    'macro-ns.main $ %{} 'FileEntry
      :defs $ {} $ 'main!
        %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () (expand-1 1)
          :examples $ []
          :schema $ :: 'Dynamic
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns macro-ns.main
          :require $ macro-ns.lib :refer $ expand-1

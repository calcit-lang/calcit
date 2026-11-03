
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |app
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'app.main
    %{} 'FileEntry
      :defs $ {}
        'Cell $ %{} 'CodeEntry
          :doc "|A union lifts into JsNullish of the same union, but not into JsNullish of one member type."
          :code $ quote $ defunion Cell String Number
          :examples $ []
        'cell $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def cell 1
          :examples $ []
          :schema $ quote app.main/Cell
        'f $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn f (v) &unit
          :examples $ []
          :schema $ quote $ Fn (JsNullish Number) (-> Unit)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () (f cell)
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main (:require)

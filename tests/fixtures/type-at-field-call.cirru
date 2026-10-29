{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |type-at-field-call
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/main!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'app.main $ %{} 'FileEntry
      :defs $ {}
        'Point $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Point (:x 'Number) (:y 'Number)
          :examples $ []
          :schema $ quote StructDef
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            println (point-x $ Point :x 1 :y 2) (map-a $ {} (:a 1))
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'map-a $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn map-a (m) (:a m)
          :examples $ []
          :schema $ quote $ Fn (Map Tag Number) (-> Dynamic)
        'point-x $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn point-x (p) (:x p)
          :examples $ []
          :schema $ quote $ Fn app.main/Point (-> Number)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main

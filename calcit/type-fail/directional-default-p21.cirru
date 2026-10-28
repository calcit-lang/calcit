
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |type-fail-directional-p21
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'type-fail-directional-p21.main/main!) (:mode :native) (:reload-fn 'type-fail-directional-p21.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'type-fail-directional-p21.main
    %{} 'FileEntry
      :defs $ {}
        'f $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn f (xs)
            map xs $ fn (x) (&+ x 1)
          :examples $ []
          :schema $ quote $ Fn (List Dynamic)
            -> $ List Number
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () println
            f $ [] |a
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns type-fail-directional-p21.main


{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |type-fail-directional-p30
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'type-fail-directional-p30.main/main!) (:mode :native) (:reload-fn 'type-fail-directional-p30.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'type-fail-directional-p30.main
    %{} 'FileEntry
      :defs $ {}
        'f $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn f (a b) (and a b)
          :examples $ []
          :schema $ quote $ Fn Bool Number (-> Number)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () println
            &+ 1 $ f false 2
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns type-fail-directional-p30.main

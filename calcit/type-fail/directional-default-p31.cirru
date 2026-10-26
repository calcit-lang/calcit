

{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |type-fail-directional-p31
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'type-fail-directional-p31.main/main!) (:mode :native) (:reload-fn 'type-fail-directional-p31.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'type-fail-directional-p31.main
    %{} 'FileEntry
      :defs $ {}
        'g $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn g (cb) (cb |a)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ [] $ :: 'Fn
              {} (:return 'Unit)
                :args $ [] 'Dynamic
        'h $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn h (x)
            println $ &+ x 1
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ [] 'Number
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () (g h)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns type-fail-directional-p31.main

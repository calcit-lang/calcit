
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |app
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/main!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {} $ :dispatch-op |app.main/Op
  :files $ {} $ 'app.main
    %{} 'FileEntry
      :defs $ {}
        'Op $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def Op (:: :sample)
          :examples $ []
          :schema $ quote Enum
        'accept $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn accept (f)
            f $ :: :sample
          :examples $ []
          :schema $ quote $ Fn
            Fn *dispatch-op $ -> Unit
            -> Unit
        'callback $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn callback (op)
            assert= (:: :sample) op
          :examples $ []
          :schema $ quote $ Fn Enum (-> Unit)
        'forward $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn forward (op) (callback op)
          :examples $ []
          :schema $ quote $ Fn *dispatch-op (-> Unit)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () (accept callback)
            forward $ :: :sample
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
          :tests $ []
            %{} 'TestEntry (:name |enum-callback-through-slot)
              :code $ quote $ assert= &unit (accept callback)
              :tags $ #{} :type-slot :unit
            %{} 'TestEntry (:name |slot-callback-forwards-to-enum)
              :code $ quote $ assert= &unit
                forward $ :: :sample
              :tags $ #{} :type-slot :unit
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main (:require)


{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |type-fail-type-slot-entry-scope
  :entries $ {}
    :default $ {} (:description |)
      :init-fn 'type-fail-type-slot-entry-scope.main/client-main!
      :mode :native
      :reload-fn 'type-fail-type-slot-entry-scope.main/reload!
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {} $ :dispatch-op |type-fail-type-slot-entry-scope.main/ClientOp
    :server $ {} (:description |)
      :init-fn 'type-fail-type-slot-entry-scope.main/server-main!
      :mode :native
      :reload-fn 'type-fail-type-slot-entry-scope.main/reload!
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {} $ :dispatch-op |type-fail-type-slot-entry-scope.main/ServerOp
  :files $ {} $ 'type-fail-type-slot-entry-scope.main
    %{} 'FileEntry
      :defs $ {}
        'ClientOp $ %{} 'CodeEntry
          :doc "|Client entry enum"
          :code $ quote $ defenum ClientOp (:client/ping)
          :examples $ []
          :schema $ :: 'EnumDef
        'ServerOp $ %{} 'CodeEntry
          :doc "|Server entry enum"
          :code $ quote $ defenum ServerOp (:server/ping)
          :examples $ []
          :schema $ :: 'EnumDef
        'accept-op $ %{} 'CodeEntry
          :doc "|Schema depends on the entry-bound type slot"
          :code $ quote $ defn accept-op (op) op
          :examples $ []
          :schema $ :: 'Fn $ {} (:return '*dispatch-op)
            :args $ [] '*dispatch-op
        'client-main! $ %{} 'CodeEntry
          :doc "|Client entry binds dispatch-op for client enums"
          :code $ quote $ defn client-main! ()
            accept-op $ :: :client/ping
            , nil
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'reload! $ %{} 'CodeEntry (:doc "|Reload handler")
          :code $ quote $ defn reload! () nil
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'server-main! $ %{} 'CodeEntry
          :doc "|Server entry binds the same slot name independently"
          :code $ quote $ defn server-main! ()
            accept-op $ :: :server/ping
            , nil
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
      :ns $ %{} 'NsEntry
        :doc "|Fixture for entry-scoped with-type-slot preprocessing"
        :code $ quote $ ns type-fail-type-slot-entry-scope.main

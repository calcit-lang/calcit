
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |demo
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'demo.main/main!) (:mode :native) (:reload-fn 'demo.main/main!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {} $ :dispatch-op 'demo.main/ViewDispatch
  :files $ {} $ 'demo.main
    %{} 'FileEntry
      :defs $ {}
        'RemoteOp $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum RemoteOp (:send String)
          :examples $ []
          :schema $ quote EnumDef
        'ViewDispatch $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def ViewDispatch
            :: :states ([]) nil
          :examples $ []
          :schema $ quote AnyEnum
        'invoke $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn invoke (f)
            hint-fn $ Fn
              Fn *dispatch-op $ -> Unit
              -> Unit
            f $ RemoteOp :send |hello
          :examples $ []
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns demo.main

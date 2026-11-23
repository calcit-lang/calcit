
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |app
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/main!) (:target :native)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {} $ :op 'app.main/Ui
  :files $ {} $ 'app.main
    %{} 'FileEntry
      :defs $ {}
        'Local $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum Local (:states Bool)
          :examples $ []
          :schema $ quote EnumDef
        'Remote $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum Remote (:remote String)
          :examples $ []
          :schema $ quote EnumDef
        'Ui $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defunion Ui Local Remote
          :examples $ []
          :schema $ quote UnionDef
        'deliver $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn deliver (callback)
            callback $ :: :states true
          :examples $ []
          :schema $ quote $ Fn
            Fn *op $ -> Unit
            -> Unit
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            deliver $ fn (value)
              hint-fn $ Fn Ui $ -> Unit
              match value
                (Local event)
                  match event $
                    :states flag
                    assert |nominal-payload flag
                (Remote event) (raise |wrong-enum)
            println |union-nominal-ok
          :examples $ []
          :schema $ quote $ Fn (-> Unit) (:features :io)
        'slot $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def slot (deftype-slot :op)
          :examples $ []
          :schema $ quote TypeSlot
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main

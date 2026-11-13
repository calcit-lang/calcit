
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |app
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/main!) (:target :native)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'app.main
    %{} 'FileEntry
      :defs $ {}
        'Cell $ %{} 'CodeEntry (:doc "|An Option member does not admit a bare nil.")
          :code $ quote $ defunion Cell String (Option Number)
          :examples $ []
        'f $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn f (v)
            match v
              (String s) s
              (Option o) |opt
          :examples $ []
          :schema $ quote $ Fn Cell (-> String)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            println $ f nil
            , &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit) (:features :io)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main (:require)

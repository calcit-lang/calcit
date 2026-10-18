
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
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            option-value $ Option :some nil
            , &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'option-value $ %{} 'CodeEntry (:doc |)
          :code $ quote $ fn (value) nil
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'Option 'Nil
            :return $ :: 'Option 'Nil
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main (:require)

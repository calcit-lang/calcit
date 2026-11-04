
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |app
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/main!) (:target :native)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'app.main $ %{} 'FileEntry
      :defs $ {}
        'Both $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defunion Both app.one/Value app.two/Value
          :examples $ []
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () (println Both) &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main (:require)
    'app.one $ %{} 'FileEntry
      :defs $ {} $ 'Value
        %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Value (:x Number)
          :examples $ []
          :schema $ quote StructDef
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.one (:require)
    'app.two $ %{} 'FileEntry
      :defs $ {} $ 'Value
        %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Value (:y String)
          :examples $ []
          :schema $ quote StructDef
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.two (:require)

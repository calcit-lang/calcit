
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |app
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/main!) (:target :native)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'app.lib $ %{} 'FileEntry
      :defs $ {}
        'Labeled $ %{} 'CodeEntry (:doc |)
          :code $ quote $ deftrait Labeled
            .label $ Fn T (-> String) (:generics T)
          :examples $ []
          :schema $ quote Trait
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.lib
    'app.main $ %{} 'FileEntry
      :defs $ {}
        'f $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn f (n)
            assert-type n lib/Labeled
            , 1
          :examples $ []
          :schema $ quote $ Fn Dynamic (-> Number)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            println $ f 1
            , &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main $ :require (app.lib :as lib)

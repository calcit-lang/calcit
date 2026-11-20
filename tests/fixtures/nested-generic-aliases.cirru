
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
      :defs $ {} $ 'call-lib
        %{} 'CodeEntry (:doc |)
          :code $ quote $ defn call-lib () (println "|Calling lib") &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit) (:features :io)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.lib
    'app.main $ %{} 'FileEntry
      :defs $ {}
        'New $ %{} 'CodeEntry (:doc |)
          :code $ quote $ deftype New (:generics T) T
          :examples $ []
        'Old $ %{} 'CodeEntry (:doc |)
          :code $ quote $ deftype Old (:generics T) T
          :examples $ []
        'consume $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn consume (x) &unit
          :examples $ []
          :schema $ quote $ Fn (New Number) (-> Unit)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            consume $ produce
            , &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit) (:features :io)
        'produce $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn produce () 1
          :examples $ []
          :schema $ quote $ Fn
            -> $ Old $ Old Number
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main
          :require $ app.lib :refer $ call-lib

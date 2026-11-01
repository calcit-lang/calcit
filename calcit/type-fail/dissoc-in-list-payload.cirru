
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |dissoc-in-list-payload
  :entries $ {} $ :default
    {}
      :description "|Proof fixture for dissoc-in keeping a known Map root (#2506)."
      :init-fn 'dissoc-in-list-payload.main/main!
      :mode :native
      :reload-fn 'dissoc-in-list-payload.main/main!
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'dissoc-in-list-payload.main
    %{} 'FileEntry
      :defs $ {}
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            remove-item
              {} $ :groups $ [] 1 2
              , 0
            , &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'remove-item $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn remove-item (data idx)
            dissoc-in data $ [] :groups idx
          :examples $ []
          :schema $ quote $ Fn
            Map Tag $ List Number
            , Number
              -> $ Map Tag $ List Number
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns dissoc-in-list-payload.main

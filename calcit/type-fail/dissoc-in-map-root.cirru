
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |dissoc-in-map-root
  :entries $ {} $ :default
    {}
      :description "|Proof fixture for dissoc-in keeping a known Map root (#2506)."
      :init-fn 'dissoc-in-map-root.main/main!
      :mode :native
      :reload-fn 'dissoc-in-map-root.main/main!
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'dissoc-in-map-root.main
    %{} 'FileEntry
      :defs $ {}
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            remove-group
              {} $ :groups $ {}
              , |synthetic
            remove-nested
              {} $ :groups $ {}
              , |synthetic
            remove-root
              {} $ :groups $ {}
              , :groups
            , &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'remove-group $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn remove-group (data id)
            dissoc-in data $ [] :groups id
          :examples $ []
          :schema $ quote $ Fn (Map Tag Dynamic) String
            -> $ Map Tag Dynamic
        'remove-nested $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn remove-nested (data id)
            dissoc-in data $ [] :groups id
          :examples $ []
          :schema $ quote $ Fn
            Map Tag $ Map String Number
            , String
              -> $ Map Tag $ Map String Number
        'remove-root $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn remove-root (data k)
            dissoc-in data $ [] k
          :examples $ []
          :schema $ quote $ Fn (Map Tag Dynamic) Tag
            -> $ Map Tag Dynamic
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns dissoc-in-map-root.main

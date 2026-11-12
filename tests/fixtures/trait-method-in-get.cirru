
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |app
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'app.min/main!) (:mode :native) (:reload-fn 'app.min/main!) (:target :native)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'app.min $ %{} 'FileEntry
      :defs $ {}
        'find-item $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn find-item (files value)
            get files $ .get-name value
          :examples $ []
          :schema $ quote $ Fn (Map String String) app.model/%item
            -> $ Option String
          :tests $ [] $ %{} 'TestEntry (:name |proven-method-inside-get)
            :code $ quote $ do
              assert= (:: :some |b.txt)
                find-item
                  {} $ |a |b.txt
                  :: :item |a
              assert= (:: :none)
                find-item
                  {} $ |a |b.txt
                  :: :item |z
            :tags $ #{} :trait-method-in-get :unit
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            println $ fn? find-item
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.min (:require)
    'app.model $ %{} 'FileEntry
      :defs $ {}
        '%item $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum %item (:item String) (:impls ItemOps)
          :examples $ []
          :schema $ quote EnumDef
        'ItemImpl $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defimpl %item ItemOps
            .get-name $ fn (self)
              hint-fn $ Fn app.model/%item $ -> String
              match self $
                :item text
                , text
          :examples $ []
          :schema $ quote Impl
        'ItemOps $ %{} 'CodeEntry (:doc |)
          :code $ quote $ deftrait ItemOps
            .get-name $ Fn app.model/%item $ -> String
          :examples $ []
          :schema $ quote Trait
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.model (:require)

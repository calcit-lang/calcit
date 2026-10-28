
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |js-global-shadow
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'js-global-shadow.main/main!) (:mode :js) (:reload-fn 'js-global-shadow.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'js-global-shadow.main $ %{} 'FileEntry
      :defs $ {}
        'imported-element $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn imported-element ()
            hint-fn $ Fn $ -> Dynamic
            Element :name |probe
          :examples $ []
          :schema $ quote Dynamic
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            hint-fn $ Fn $ -> Dynamic
            resolve-ctor
          :examples $ []
          :schema $ quote Dynamic
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! ()
            hint-fn $ Fn $ -> Unit
            , &unit
          :examples $ []
          :schema $ quote Dynamic
        'resolve-ctor $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn resolve-ctor ()
            hint-fn $ Fn (-> Dynamic) (:features :js-ffi)
            if (exists? js/Element) js/Element js/Error
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic) (:features :js-ffi)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns js-global-shadow.main
          :require $ js-global-shadow.schema :refer $ Element
    'js-global-shadow.schema $ %{} 'FileEntry
      :defs $ {} $ 'Element
        %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Element (:name 'String)
          :examples $ []
          :schema $ quote StructDef
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns js-global-shadow.schema

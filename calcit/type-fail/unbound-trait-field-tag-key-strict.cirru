
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |type-fail-unbound-trait-field-tag-key-strict
  :entries $ {} $ :default
    {}
      :description "|Strict preprocessing fixture for a Tag host key on an unbound trait field type."
      :init-fn 'type-fail-unbound-trait-field-tag-key-strict.main/main!
      :mode :js
      :reload-fn 'type-fail-unbound-trait-field-tag-key-strict.main/reload!
      :target :node
      :feature-policy $ {} $ :js-ffi :error
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'type-fail-unbound-trait-field-tag-key-strict.main
    %{} 'FileEntry
      :defs $ {}
        'Host $ %{} 'CodeEntry
          :doc "|Names a field type that resolves to no definition, so the field reads as a type variable."
          :code $ quote $ deftrait Host (:style 'MissingStyle)
          :examples $ []
          :ffi $ {} (:backend :js) (:kind :external-object)
          :schema $ quote Trait
        'main! $ %{} 'CodeEntry
          :doc "|A Tag key on a type-variable receiver would write a raw `:width` property, so strict rejects it."
          :code $ quote $ defn main! ()
            let
                host $ unsafe-coerce (js-object) type-fail-unbound-trait-field-tag-key-strict.main/Host
                style $ host :style
              js-set style :width |100px
              , &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit) (:features :js-ffi)
        'reload! $ %{} 'CodeEntry (:doc "|Reload handler.")
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
      :ns $ %{} 'NsEntry (:doc "|Strict unbound trait field host key fixture.")
        :code $ quote $ ns type-fail-unbound-trait-field-tag-key-strict.main

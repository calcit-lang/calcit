
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |type-fail-unsafe-coerce-scoped-strict
  :entries $ {} $ :default
    {}
      :description "|Strict preprocessing fixture for source-definition capability isolation."
      :init-fn 'type-fail-unsafe-coerce-scoped-strict.main/main!
      :mode :native
      :reload-fn 'type-fail-unsafe-coerce-scoped-strict.main/reload!
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'type-fail-unsafe-coerce-scoped-strict.main
    %{} 'FileEntry
      :defs $ {}
        '*unsafe $ %{} 'CodeEntry
          :doc "|Unmarked top-level initialization must not inherit the caller's JS FFI capability."
          :code $ quote $ defref *unsafe (unsafe-coerce 1 'String)
          :examples $ []
          :schema $ quote $ Ref String
        'coerce-host $ %{} 'CodeEntry
          :doc "|A marked adapter references the unmarked top-level atom."
          :code $ quote $ defn coerce-host (value) (deref *unsafe)
          :examples $ []
          :schema $ quote $ Fn Dynamic (-> String) (:features :js-ffi)
        'main! $ %{} 'CodeEntry
          :doc "|The entry reaches the atom only through the marked adapter."
          :code $ quote $ defn main! () (coerce-host 1)
          :examples $ []
          :schema $ quote $ Fn (-> String)
        'reload! $ %{} 'CodeEntry (:doc "|Reload handler.")
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
      :ns $ %{} 'NsEntry (:doc "|Strict scoped unsafe-coerce fixture.")
        :code $ quote $ ns type-fail-unsafe-coerce-scoped-strict.main

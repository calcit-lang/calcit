{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --full` first. Manual edits must follow format and schema conventions, then run `calcit edit format`.") (:package |type-fail-untyped-js-object-access-strict)
  :entries $ {}
    :default $ {} (:description "|Strict preprocessing fixture for literal access on a bare JsObject.") (:init-fn 'type-fail-untyped-js-object-access-strict.main/main!) (:mode :js) (:reload-fn 'type-fail-untyped-js-object-access-strict.main/reload!) (:target :node)
      :feature-policy $ {} (:js-ffi :error)
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'type-fail-untyped-js-object-access-strict.main $ %{} 'FileEntry
      :defs $ {}
        'main! $ %{} 'CodeEntry (:doc "|A known member on a bare host object must use an external-object trait.")
          :code $ quote
            defn main! () $ let
                host $ js-object
              .-length host
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ []
              :features $ #{} :js-ffi
        'reload! $ %{} 'CodeEntry (:doc "|Reload handler.")
          :code $ quote (defn reload! () &unit)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ []
      :ns $ %{} 'NsEntry (:doc "|Strict untyped JavaScript object access fixture.")
        :code $ quote (ns type-fail-untyped-js-object-access-strict.main)

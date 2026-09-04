{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --full` first. Manual edits must follow format and schema conventions, then run `calcit edit format`.") (:package |type-fail-js-nullish-predicate-strict)
  :entries $ {}
    :default $ {} (:description "|Strict preprocessing fixture for a legacy nil predicate on a nullable JavaScript host value.") (:init-fn 'type-fail-js-nullish-predicate-strict.main/main!) (:mode :js) (:reload-fn 'type-fail-js-nullish-predicate-strict.main/reload!) (:target :node)
      :feature-policy $ {} (:js-ffi :error)
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'type-fail-js-nullish-predicate-strict.main $ %{} 'FileEntry
      :defs $ {}
        'main! $ %{} 'CodeEntry (:doc "|A nullable host value must use a JavaScript-specific presence predicate.")
          :code $ quote
            defn main! () $ let
                host $ js/process.argv
              nil? host
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ []
              :features $ #{} :js-ffi
        'reload! $ %{} 'CodeEntry (:doc "|Reload handler.")
          :code $ quote (defn reload! () &unit)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ []
      :ns $ %{} 'NsEntry (:doc "|Strict nullable JavaScript predicate fixture.")
        :code $ quote (ns type-fail-js-nullish-predicate-strict.main)

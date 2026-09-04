{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --full` first. Manual edits must follow format and schema conventions, then run `calcit edit format`.") (:package |type-fail-filter-predicate)
  :entries $ {}
    :default $ {} (:description "|Filter predicates must return Bool.") (:init-fn 'type-fail-filter-predicate.main/main!) (:mode :native) (:reload-fn 'type-fail-filter-predicate.main/reload!)
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'type-fail-filter-predicate.main $ %{} 'FileEntry
      :defs $ {}
        'identity-number $ %{} 'CodeEntry (:doc "|Deliberately invalid predicate contract.")
          :code $ quote
            defn identity-number (x) x
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'Number
        'main! $ %{} 'CodeEntry (:doc "|Pass a Number-returning callback where filter requires Bool.")
          :code $ quote
            defn main! () $ filter ([] 1 2 3) identity-number
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ []
              :return $ :: 'List 'Number
        'reload! $ %{} 'CodeEntry (:doc "|Reload handler.")
          :code $ quote
            defn reload! () &unit
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ []
      :ns $ %{} 'NsEntry (:doc "|Filter predicate mismatch fixture.")
        :code $ quote (ns type-fail-filter-predicate.main)

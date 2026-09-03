
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --full` first. Manual edits must follow format and schema conventions, then run `calcit edit format`.") (:package |type-fail-erased-generic-relation-strict)
  :entries $ {}
    :default $ {} (:description "|Strict preprocessing fixture for a generic relation erased by Dynamic.") (:init-fn 'type-fail-erased-generic-relation-strict.main/main!) (:mode :native) (:reload-fn 'type-fail-erased-generic-relation-strict.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'type-fail-erased-generic-relation-strict.main $ %{} 'FileEntry
      :defs $ {}
        'compare-open $ %{} 'CodeEntry (:doc "|Dynamic input must be narrowed before entering the homogeneous generic equality relation.")
          :code $ quote
            defn compare-open (value) (= value 1)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'Dynamic
        'main! $ %{} 'CodeEntry (:doc "|Entry that makes the erased generic relation reachable.")
          :code $ quote
            defn main! () $ compare-open 1
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ []
        'reload! $ %{} 'CodeEntry (:doc "|Reload handler.")
          :code $ quote
            defn reload! () &unit
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ []
      :ns $ %{} 'NsEntry (:doc "|Strict erased-generic-relation fixture.")
        :code $ quote (ns type-fail-erased-generic-relation-strict.main)

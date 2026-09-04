
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --full` first. Manual edits must follow format and schema conventions, then run `calcit edit format`.") (:package |type-fail-collection-member-contract)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'type-fail-collection-member-contract.main/main!) (:mode :native) (:reload-fn 'type-fail-collection-member-contract.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'type-fail-collection-member-contract.main $ %{} 'FileEntry
      :defs $ {}
        'main! $ %{} 'CodeEntry (:doc "|Reject mismatched collection lookup, membership, key, and association arguments")
          :code $ quote
            defn main! ()
              get ([] 1 2) :bad
              get (&{} :a 1) 0
              includes? (#{} 1 2) :bad
              includes? (&{} :a 1) :a
              includes? |abc 1
              contains? ([] 1 2) :bad
              contains? (&{} :a 1) 0
              contains? (#{} 1 2) :bad
              assoc ([] 1 2) :bad 3
              assoc ([] 1 2) 0 :bad
              assoc (&{} :a 1) 0 2
              assoc (&{} :a 1) :a :bad
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        'reload! $ %{} 'CodeEntry (:doc "|Reload handler")
          :code $ quote
            defn reload! () nil
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
      :ns $ %{} 'NsEntry (:doc "|Namespace for collection member contract mismatches")
        :code $ quote (ns type-fail-collection-member-contract.main)

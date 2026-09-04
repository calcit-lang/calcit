{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |type-fail-collection-member-contract) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'type-fail-collection-member-contract.main/main!) (:mode :native) (:reload-fn 'type-fail-collection-member-contract.main/reload!)
      :modules $ []
      :type-slots $ {}
  :files $ {}
    |type-fail-collection-member-contract.main $ %{} 'FileEntry
      :defs $ {}
        |main! $ %{} 'CodeEntry (:doc "|Reject mismatched collection lookup and membership arguments")
          :code $ quote
            defn main! ()
              get ([] 1 2) :bad
              get (&{} :a 1) 0
              includes? (#{} 1 2) :bad
              includes? (&{} :a 1) :a
              includes? |abc 1
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        |reload! $ %{} 'CodeEntry (:doc "|Reload handler")
          :code $ quote
            defn reload! () nil
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
      :ns $ %{} 'NsEntry (:doc "|Namespace for collection member contract mismatches")
        :code $ quote (ns type-fail-collection-member-contract.main)


{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |type-fail-update-collection-contract) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'type-fail-update-collection-contract.main/main!) (:mode :native) (:reload-fn 'type-fail-update-collection-contract.main/reload!)
      :modules $ []
      :type-slots $ {}
  :files $ {}
    |type-fail-update-collection-contract.main $ %{} 'FileEntry
      :defs $ {}
        |main! $ %{} 'CodeEntry (:doc "|Reject mismatched List/Map update keys and callbacks")
          :code $ quote
            defn main! ()
              update ([] 1 2) :bad inc
              update (&{} :a 1) :a string-id
              update ([] 1 2) 0 $ fn (x)
                str x
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
        |string-id $ %{} 'CodeEntry (:doc "|Deliberately incompatible updater")
          :code $ quote
            defn string-id (x) x
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'String)
              :args $ [] 'String
      :ns $ %{} 'NsEntry (:doc "|Namespace for update collection contract mismatch")
        :code $ quote (ns type-fail-update-collection-contract.main)

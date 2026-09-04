
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |type-fail-slice-receiver-trait) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'type-fail-slice-receiver-trait.main/main!) (:mode :native) (:reload-fn 'type-fail-slice-receiver-trait.main/reload!)
      :modules $ []
      :type-slots $ {}
  :files $ {}
    |type-fail-slice-receiver-trait.main $ %{} 'FileEntry
      :defs $ {}
        |main! $ %{} 'CodeEntry (:doc "|Reject a receiver that does not implement Sliceable")
          :code $ quote
            defn main! () (slice 1 0 1) nil
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
      :ns $ %{} 'NsEntry (:doc "|Namespace for slice receiver trait mismatch")
        :code $ quote (ns type-fail-slice-receiver-trait.main)

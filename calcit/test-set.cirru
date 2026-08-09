
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |test-set) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'test-set.main/main!) (:mode :native) (:reload-fn 'test-set.main/reload!)
      :modules $ [] |./util.cirru
      :type-slots $ {}
  :files $ {}
    |test-set.main $ %{} 'FileEntry
      :defs $ {}
        |main! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn main! () (log-title "|Testing set") (test-method-dispatch) (do true)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        |test-method-dispatch $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn test-method-dispatch () $ assert= (#{} 1 2 3)
              .add (#{} 1 2) 3
          :examples $ []
          :schema $ :: 'Dynamic
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote
          ns test-set.main $ :require
            util.core :refer $ log-title

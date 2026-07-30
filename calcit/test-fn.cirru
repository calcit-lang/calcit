
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |test-fn) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:init-fn |test-fn.main/main!) (:mode :native) (:reload-fn |test-fn.main/reload!)
      :modules $ []
      :type-slots $ {}
  :files $ {}
    |test-fn.main $ %{} :FileEntry
      :defs $ {}
        |main! $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defn main! () (log-title "|Testing fn")
              let
                  f1 identity
                  f2 &+
                  _ $ assert-type f1
                    :: :fn $ {} (:return 'T)
                      :generics $ [] 'T
                      :args $ [] 'T
                  _ $ assert-type f2
                    :: :fn $ {} (:return :number)
                      :args $ [] :number :number
                assert= 1 $ f1 1
                assert= 3 $ f2 1 2
                assert= 3 $ apply f2 ([] 1 2)
          :examples $ []
      :ns $ %{} :NsEntry (:doc |)
        :code $ quote
          ns test-fn.main $ :require
            util.core :refer $ log-title

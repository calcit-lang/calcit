
{} (:about "|Machine-generated snapshot. AI AGENTS: never edit this file directly — changes will be overwritten on recompile. Inspect via `cr query`; modify via `cr edit` / `cr tree`. MANDATORY first step: run `cr docs agents --full`.") (:package |test-fn)
  :configs $ {} (:init-fn |test-fn.main/main!) (:reload-fn |test-fn.main/reload!) (:version |0.0.0)
    :modules $ []
  :entries $ {}
  :files $ {}
    |test-fn.main $ %{} :FileEntry
      :defs $ {}
        |main! $ %{} :CodeEntry (:doc |) (:schema nil)
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

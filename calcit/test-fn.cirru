
{} (:about "|file is generated - never edit directly; learn cr edit/tree workflows before changing") (:package |test-fn)
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
                  _ $ assert-type f1 :fn
                  _ $ assert-type f2 :fn
                assert= 1 $ f1 1
                assert= 3 $ f2 1 2
                assert= 3 $ apply f2 ([] 1 2)
          :examples $ []
      :ns $ %{} :CodeEntry (:doc |) (:schema nil)
        :code $ quote
          ns test-fn.main $ :require
            util.core :refer $ log-title
        :examples $ []

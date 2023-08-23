
{} (:package |test-gynienic)
  :configs $ {} (:init-fn |test-gynienic.main/main!) (:reload-fn |test-gynienic.main/reload!)
  :files $ {}
    |test-gynienic.lib $ {}
      :defs $ {}
        |add-11 $ %{} :CodeEntry
          :code $ quote
            defmacro add-11 (a b)
              let
                  c 11
                println "\"internal c:" a b c
                quasiquote $ do (println "\"c is:" c)
                  [] (~ a) (~ b) c (~ c) (add-2 8)
          :doc |
        |add-2 $ %{} :CodeEntry
          :code $ quote
            defn add-2 (x) (&+ x 2)
          :doc |
      :ns $ %{} :CodeEntry
        :code $ quote (:ns test-gynienic.lib)
        :doc |
    |test-gynienic.main $ {}
      :configs $ {}
      :defs $ {}
        |main! $ %{} :CodeEntry
          :code $ quote
            defn main! () $ try-hygienic
          :doc |
        |try-hygienic $ %{} :CodeEntry
          :code $ quote
            defn try-hygienic () (println "|Testing gynienic")
              let
                  c 4
                assert= (add-11 1 2) ([] 1 2 4 11 10)
                , true
          :doc |
      :ns $ %{} :CodeEntry
        :code $ quote
          ns test-gynienic.main $ :require
            [] test-gynienic.lib :refer $ [] add-11
        :doc |
      :proc $ quote ()

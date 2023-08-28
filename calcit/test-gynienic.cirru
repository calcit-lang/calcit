
{} (:package |test-gynienic)
  :configs $ {} (:init-fn |test-gynienic.main/main!) (:reload-fn |test-gynienic.main/reload!)
  :files $ {}
    |test-gynienic.lib $ {}
      :defs $ {}
        |add-11 $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro add-11 (a b)
              let
                  c 11
                println "\"internal c:" a b c
                quasiquote $ do (println "\"c is:" c)
                  [] (~ a) (~ b) c (~ c) (add-2 8)
        |add-2 $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn add-2 (x) (&+ x 2)
      :ns $ %{} :CodeEntry (:doc |)
        :code $ quote (:ns test-gynienic.lib)
    |test-gynienic.main $ {}
      :configs $ {}
      :defs $ {}
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! () $ try-hygienic
        |try-hygienic $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn try-hygienic () (println "|Testing gynienic")
              let
                  c 4
                assert= (add-11 1 2) ([] 1 2 4 11 10)
                , true
      :ns $ %{} :CodeEntry (:doc |)
        :code $ quote
          ns test-gynienic.main $ :require
            [] test-gynienic.lib :refer $ [] add-11
      :proc $ quote ()

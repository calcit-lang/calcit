
{} (:package |test-hygienic)
  :configs $ {} (:init-fn |test-hygienic.main/main!) (:reload-fn |test-hygienic.main/reload!)
  :files $ {}
    |test-hygienic.lib $ %{} :FileEntry
      :defs $ {}
        |add-11 $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro add-11 (a b)
              let
                  c 11
                println "\"internal c:" a b c
                quasiquote $ do (println "\"c is:" c)
                  [] (~ a) (~ b) c (~ c) (add-2 8)
        |add-11-safe $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro add-11-safe (a b)
              with-gensyms (c)
                quasiquote $ do
                  &let (~c 11)
                    [] (~ a) (~ b) ~c
        |add-2 $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn add-2 (x) (&+ x 2)
      :ns $ %{} :CodeEntry (:doc |)
        :code $ quote (:ns test-hygienic.lib)
    |test-hygienic.main $ %{} :FileEntry
      :defs $ {}
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! () $ try-hygienic
        |try-hygienic $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn try-hygienic () (println "|Testing hygienic")
              let
                  c 4
                assert= (add-11 1 2) ([] 1 2 4 11 10)
                assert= (add-11-safe 1 2) ([] 1 2 11)
                , true
      :ns $ %{} :CodeEntry (:doc |)
        :code $ quote
          ns test-hygienic.main $ :require
            test-hygienic.lib :refer $ add-11 add-11-safe

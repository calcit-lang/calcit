
{} (:package |test-gynienic)
  :configs $ {} (:init-fn |test-gynienic.main/main!) (:reload-fn |test-gynienic.main/reload!)
  :files $ {}
    |test-gynienic.lib $ {}
      :ns $ quote
        :ns test-gynienic.lib
      :defs $ {}
        |add-2 $ quote
          defn add-2 (x) (&+ x 2)
        |add-11 $ quote
          defmacro add-11 (a b)
            let
                c 11
              println "\"internal c:" a b c
              quasiquote $ do (println "\"c is:" c)
                [] (~ a) (~ b) (, c) (~ c) (add-2 8)
    |test-gynienic.main $ {}
      :ns $ quote
        ns test-gynienic.main $ :require ([] test-gynienic.lib :refer $ [] add-11)
      :defs $ {}
        |try-hygienic $ quote
          defn try-hygienic ()
            println "|Testing gynienic"
            let
                c 4
              assert=
                add-11 1 2
                [] 1 2 4 11 10
              , true

        |main! $ quote
          defn main! ()
            try-hygienic

      :proc $ quote ()
      :configs $ {}

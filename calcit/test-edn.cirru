
{} (:package |test-edn)
  :configs $ {} (:init-fn |test-edn.main/main!) (:reload-fn |test-edn.main/reload!)
  :files $ {}
    |test-edn.main $ %{} :FileEntry
      :defs $ {}
        |log-title $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn log-title (title) (println) (println title) (println)
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! () (log-title "|Testing edn")
              test-edn
              test-edn-comment
        |test-edn $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-edn ()
              let
                  Person $ new-record 'Person :name :age
                  edn-demo "|%{} :Person (:age 23) (:name |Chen)"
                ; "no stable order"
                assert=
                  count $ to-lispy-string
                    %{} Person (:name |Chen) (:age 23)
                  count "|(%{} :Person (:name |Chen) (:age 23))"
                ; "no stable order"
                assert= (count edn-demo)
                  count $ trim
                    format-cirru-edn $ %{} Person (:name |Chen) (:age 23)
                assert= (parse-cirru-edn edn-demo)
                  %{} Person (:name |Chen) (:age 23)
                assert= 'a $ parse-cirru-edn "|do 'a"
                assert=
                  {} $ :code
                    cirru-quote $ + 1 2 3
                  parse-cirru-edn "|{} $ :code $ quote $ + 1 2 3"
                assert= (:: :a 1) (parse-cirru-edn "|:: :a 1")
                assert= :cirru-quote $ type-of (parse-cirru "|a b")
                assert= "|{} $ :code\n  quote $ + 1 2 3" $ trim
                  format-cirru-edn $ {}
                    :code $ :: 'quote ([] |+ |1 |2 |3)
                assert= "|{} $ :code\n  quote $ + 1 2 3" $ trim
                  format-cirru-edn $ {}
                    :code $ cirru-quote (+ 1 2 3)
                assert= "|[] 'a" $ trim
                  format-cirru-edn $ [] 'a
                assert= "|do nil" $ trim (format-cirru-edn nil)
                assert= "|do 's" $ trim (format-cirru-edn 's)
                assert=
                  trim $ format-cirru-edn
                    {} (:a 1) (:b 2) (:Z 3) (:D 4)
                  , "|{} (:D 4) (:Z 3) (:a 1) (:b 2)"
                assert=
                  trim $ format-cirru-edn
                    {} (|a 1) (|b 2) (|Z 3) (|D 4)
                  , "|{} (|D 4) (|Z 3) (|a 1) (|b 2)"
                assert=
                  trim $ format-cirru-edn
                    {} (:c 2) (:a1 0)
                      :b $ [] 1 2
                      :a 1
                  , "|{} (:a 1) (:a1 0) (:c 2)\n  :b $ [] 1 2"
                assert= "|:: :&core-list-class $ [] 1 2 3" $ trim
                  format-cirru-edn $ :: &core-list-class ([] 1 2 3)
                assert= "|:: :test" $ trim
                  format-cirru-edn $ :: :test
                assert= "|:: :test :a :b" $ trim
                  format-cirru-edn $ :: :test :a :b

        |test-edn-comment $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-edn-comment ()
              log-title "|Testing edn comment"
              assert=
                [] 1 2 (; comment) 3
                parse-cirru-edn "|[] 1 2 (; comment) 3"
              assert=
                {}
                  :a 1
                  :b 2
                  ; comment
                parse-cirru-edn "|{} (:a 1) (:b 2)"

              assert=
                :: :a 1
                parse-cirru-edn "|:: :a (; comment) 1"

      :ns $ %{} :CodeEntry (:doc |)
        :code $ quote
          ns test-edn.main $ :require
            [] util.core :refer $ [] inside-eval:

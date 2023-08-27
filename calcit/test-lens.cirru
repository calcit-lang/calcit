
{} (:package |test-lens)
  :configs $ {} (:init-fn |test-lens.main/main!) (:reload-fn |test-lens.main/reload!)
  :files $ {}
    |test-lens.main $ {}
      :configs $ {}
      :defs $ {}
        |log-title $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn log-title (title) (println) (println title) (println)
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! () (log-title "|Testing lens") (test-lens) (do true)
        |test-lens $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-lens ()
              assert=
                assoc-in
                  {} $ :a
                    {} $ :b ({})
                  [] :a :b
                  , 10
                {} $ :a
                  {} $ :b 10
              assert=
                assoc-in
                  {} $ :a ([] 1 2 3)
                  [] :a 1
                  , 10
                {} $ :a ([] 1 10 3)
              assert=
                update-in
                  {} $ :a
                    {} $ :b
                      {} $ :c 2
                  [] :a :b :c
                  , inc
                {} $ :a
                  {} $ :b
                    {} $ :c 3
              assert=
                update-in
                  {} $ :a ([] 1 2 3)
                  [] :a 1
                  , inc
                {} $ :a ([] 1 3 3)
              assert=
                update-in
                  {} $ :a (:: 'quote 1)
                  [] :a 1
                  , inc
                {} $ :a (:: 'quote 2)
              assert=
                dissoc-in
                  {} $ :a
                    {} $ :b
                      {} $ :c 2
                  [] :a :b :c
                {} $ :a
                  {} $ :b ({})
              assert=
                dissoc-in
                  {} $ :a ([] 1 2 3)
                  [] :a 1
                {} $ :a ([] 1 3)
              assert=
                get-in
                  {} $ :a
                    {} $ :b
                      {} $ :c 3
                  [] :a :b :c
                , 3
              assert=
                get-in
                  {} $ :a ([] 1 2 3)
                  [] :a 1
                , 2
              assert=
                assoc-in nil ([] :a :b :c) 10
                {} $ :a
                  {} $ :b
                    {} $ :c 10
              assert= true $ contains-in?
                &{} :a $ [] 1 2 3
                [] :a 1
              assert= false $ contains-in?
                &{} :a $ [] 1 2 3
                [] :a 3
              assert= false $ contains-in?
                &{} :a $ [] 1 2 3
                [] :b 1
              assert= true $ contains-in?
                [] 1 2 $ [] 3 4
                [] 2 1
              assert= false $ contains-in?
                [] 1 2 $ [] 3 4
                [] 2 2
              assert= false $ contains-in?
                [] 1 2 $ [] 3 4
                [] 3 2
              assert= true $ contains-in?
                {} $ :a (:: 'quote 1)
                [] :a 1
              assert= true $ contains-in?
                :: :a :b $ [] 1 2 3
                [] 2 2
      :ns $ %{} :CodeEntry (:doc |)
        :code $ quote
          ns test-lens.main $ :require
      :proc $ quote ()

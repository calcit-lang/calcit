
{} (:package |test-list)
  :configs $ {} (:init-fn |test-list.main/main!) (:reload-fn |test-list.main/reload!)
    :modules $ [] |./util.cirru
  :files $ {}
    |test-list.main $ {}
      :ns $ quote
        ns test-list.main $ :require
          util.core :refer $ log-title inside-nim:
      :defs $ {}

        |test-list $ quote
          defn test-list ()
            let
                a $ [] 1 2 3
              assert= a $ [] 1 2 3
              assert= (prepend a 4) $ [] 4 1 2 3
              assert= (append a 4) $ [] 1 2 3 4
              assert= (conj a 4) $ [] 1 2 3 4
              assert= 1 (first a)
              assert= 3 (last a)
              assert-detect nil? (first $ [])
              assert-detect nil? (last $ [])
              assert= (rest a) $ [] 2 3
              assert-detect nil? (rest $ [])
              assert= (butlast a) ([] 1 2)
              assert-detect nil? (butlast $ [])
              assert= (range 0) $ []
              assert= (range 1) $ [] 0
              assert= (range 4) $ [] 0 1 2 3
              assert= (range 4 5) $ [] 4
              assert= (range 4 10) $ [] 4 5 6 7 8 9
              assert= (slice (range 10) 0 10) (range 10)
              assert= (slice (range 10) 5 7) ([] 5 6)
              assert=
                concat (range 10) (range 4)
                [] 0 1 2 3 4 5 6 7 8 9 0 1 2 3
              assert=
                concat $ [] 1 2 3
                [] 1 2 3
              assert "|concat lists" $ =
                concat ([] 1 2) ([] 4 5) ([] 7 8)
                [] 1 2 4 5 7 8
              assert=
                assoc (range 10) (, 4 55)
                [] 0 1 2 3 55 5 6 7 8 9
              assert=
                dissoc (range 10) 4
                [] 0 1 2 3 5 6 7 8 9
              assert= (take 4 $ range 10) $ [] 0 1 2 3
              assert= (drop 4 $ range 10) ([] 4 5 6 7 8 9)
              assert |reverse $ =
                reverse $ [] |a |b |c |d |e
                [] |e |d |c |b |a

              assert=
                mapcat
                  fn (x) (range x)
                  [] 1 2 3 4
                [] 0 0 1 0 1 2 0 1 2 3

              assert=
                mapcat
                  fn (x) (range x)
                  [] 1
                [] 0

              assert=
                mapcat
                  fn (x) (range x)
                  []
                []

              assert=
                map identity $ range 10
                range 10

              assert=
                map inc $ #{} 1 2 3
                #{} 2 3 4

              assert=
                map-indexed (fn (idx x) ([] idx (&str x))) (range 3)
                []
                  [] 0 |0
                  [] 1 |1
                  [] 2 |2

              assert=
                filter (fn (x) (&> x 3)) (range 10)
                [] 4 5 6 7 8 9

              assert=
                filter-not (fn (x) (&> x 3)) (range 10)
                [] 0 1 2 3

              assert-detect identity $ <= 0
                index-of (range 10) $ rand-nth $ range 10

              assert-detect nil? $ rand-nth ([])

              assert-detect identity $ contains? (range 10) 6
              assert-detect not $ contains? (range 10) 16

              assert-detect identity $ has-index? (range 4) 3
              assert-detect not $ has-index? (range 4) 4
              assert-detect not $ has-index? (range 4) -1

              assert=
                update ({} (:a 1)) :a $ \ + % 10
                {} (:a 11)

              assert=
                update ({} (:a 1)) :c $ \ + % 10
                {} (:a 1)

              assert=
                update (range 4) 1 $ \ + % 10
                [] 0 11 2 3
              assert=
                update (range 4) 11 $ \ + % 10
                range 4

              assert= 6
                find
                  fn (x) (> x 5)
                  range 10

              assert= 6
                find-index
                  fn (x) (> x 5)
                  range 10

        |test-groups $ quote
          defn test-groups ()

            assert=
              group-by
                \ rem % 3
                range 10
              {}
                0 $ [] 0 3 6 9
                1 $ [] 1 4 7
                2 $ [] 2 5 8

            assert=
              frequencies $ [] 1 2 2 3 3 3
              {}
                1 1
                2 2
                3 3

            assert=
              section-by 2 $ range 10
              []
                [] 0 1
                [] 2 3
                [] 4 5
                [] 6 7
                [] 8 9
            assert=
              section-by 3 $ range 10
              []
                [] 0 1 2
                [] 3 4 5
                [] 6 7 8
                [] 9

        |test-comma $ quote
          assert=
            [] 1 2 3 4
            [,] 1 , 2 , 3 , 4

        |test-every $ quote
          defn test-every ()
            let
                data $ [] 1 2 3 4
              assert-detect not $ every?
                fn (x) (&> x 1)
                , data
              assert-detect identity $ every?
                fn (x) (&> x 0)
                , data
              assert-detect identity $ any?
                fn (x) (&> x 3)
                , data
              assert-detect not $ any?
                fn (x) (&> x 4)
                , data

            assert-detect some? 1
            assert-detect not $ some? nil

        |test-foldl $ quote
          defn test-foldl ()
            assert= 1 $ get ([] 1 2 3) 0
            assert= 6 $ foldl ([] 1 2 3) 0 &+
            assert= (+ 1 2 3 4 (+ 5 6 7)) 28
            assert= -1 (- 1 2)
            assert= -7 (- 4 5 6)
            assert= 91 (- 100 $ - 10 1)
            assert-detect identity $ foldl-compare ([] 1 2) 0 &<
            assert-detect identity (< 1 2 3 4)
            assert-detect not (< 3 2)
            assert= (* 2 3) 6
            assert= (* 2 3 4) 24
            assert= (/ 2 3) (/ 4 6)
            assert= (/ 2 3 4) (/ 1 6)

            assert=
              reduce + 2 ([] 3 4 5)
              , 14

        |test-apply $ quote
          defn test-apply ()
            assert= 10 $ apply + $ [] 1 2 3 4
            assert= 10 $ + & $ [] 1 2 3 4

        |test-join $ quote
          fn ()
            assert= |1-2-3-4 $ join-str |- $ [] 1 2 3 4
            assert= | $ join-str |- $ []
            assert=
              [] 1 10 2 10 3 10 4
              join 10 $ [] 1 2 3 4
            assert= ([]) $ join 10 $ []

        |test-repeat $ quote
          fn ()
            assert=
              repeat 5 :a
              [] :a :a :a :a :a
            assert=
              interleave ([] :a :b :c :d) ([] 1 2 3 4 5)
              [] :a 1 :b 2 :c 3 :d 4

        |test-sort $ quote
          fn ()
            assert=
              sort
                [] 4 3 2 1
                \ &- % %2
              [] 1 2 3 4

        |*counted $ quote
          defatom *counted 0

        |test-doseq $ quote
          fn ()
            log-title "|Testing doseq"

            inside-nim:
              =
                macroexpand $ quote
                  &doseq (n (range 5))
                    echo |doing: n
                    swap! *counted &+ n
                quote
                  apply
                    defn doseq-fn% (xs)
                      if (empty? xs) nil
                        &let (n (first xs))
                          echo |doing: n
                          swap! *counted &+ n
                          recur (rest xs)
                    [] (range 5)


            &doseq (n (range 5))
              swap! *counted &+ n

            assert= 10 (deref *counted)
            assert= 10 @*counted

        |test-let[] $ quote
          fn ()
            log-title "|Testing let[]"

            inside-nim:
              echo $ macroexpand $ quote
                let[] (a b c & d) ([] 1 2 3 4 5)
                  echo a
                  echo b
                  echo c
                  echo d
            let[] (a b c & d) ([] 1 2 3 4 5)
              assert= 1 a
              assert= 2 b
              assert= 3 c
              assert= ([] 4 5) d

        |main! $ quote
          defn main! ()

            log-title "|Testing list"
            test-list

            log-title "|Testing foldl"
            test-foldl

            log-title "|Testing every/any"
            test-every

            log-title "|Testing groups"
            test-groups

            log-title "|Testing apply"
            test-apply

            log-title "|Testing join"
            test-join

            log-title "|Testing repeat"
            test-repeat

            log-title "|Testing sort"
            test-sort

            test-doseq
            test-let[]

            do true

      :proc $ quote ()
      :configs $ {} (:extension nil)

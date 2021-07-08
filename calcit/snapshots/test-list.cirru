
{} (:package |test-list)
  :configs $ {} (:init-fn |test-list.main/main!) (:reload-fn |test-list.main/reload!)
    :modules $ [] |./util.cirru
  :files $ {}
    |test-list.main $ {}
      :ns $ quote
        ns test-list.main $ :require
          util.core :refer $ log-title inside-eval:
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
                &list:concat (range 10) (range 4)
                [] 0 1 2 3 4 5 6 7 8 9 0 1 2 3
              assert=
                &list:concat $ [] 1 2 3
                [] 1 2 3
              assert "|&list:concat lists" $ =
                &list:concat ([] 1 2) ([] 4 5) ([] 7 8)
                [] 1 2 4 5 7 8
              assert "|concat lists" $ =
                concat ([] 1 2) ([] 4 5) ([] 7 8)
                [] 1 2 4 5 7 8
              
              assert=
                [] 3 4 5 2 3
                &list:concat
                  slice ([] 1 2 3 4 5 6) 2 5
                  slice ([] 1 2 3 4 5 6) 1 3
              assert=
                assoc (range 10) (, 4 55)
                [] 0 1 2 3 55 5 6 7 8 9
              assert=
                dissoc (range 10) 4
                [] 0 1 2 3 5 6 7 8 9
              assert= (take (range 10) 4) $ [] 0 1 2 3
              assert= (take-last (range 10) 4) $ [] 6 7 8 9
              assert= (.take-last (range 10) 4) $ [] 6 7 8 9
              assert= (take-last (range 3) 4) $ [] 0 1 2
              assert= (drop (range 10) 4) ([] 4 5 6 7 8 9)
              assert |reverse $ =
                .reverse $ [] |a |b |c |d |e
                [] |e |d |c |b |a

              assert=
                mapcat
                  [] 1 2 3 4
                  fn (x) (range x)
                [] 0 0 1 0 1 2 0 1 2 3

              assert=
                mapcat
                  [] 1
                  fn (x) (range x)
                [] 0

              assert=
                mapcat
                  []
                  fn (x) (range x)
                []

              assert=
                map (range 10) identity
                range 10

              assert=
                map (#{} 1 2 3) inc
                #{} 2 3 4

              assert=
                map-indexed (range 3)
                  fn (idx x) ([] idx (&str x))
                []
                  [] 0 |0
                  [] 1 |1
                  [] 2 |2

              assert=
                filter (range 10)
                  fn (x) (&> x 3)
                [] 4 5 6 7 8 9

              assert=
                filter-not (range 10)
                  fn (x) (&> x 3)
                [] 0 1 2 3

              assert-detect identity $ <= 0
                index-of (range 10) $ rand-nth $ range 10

              assert= nil $ rand-nth ([])
              assert= nil (;nil anything)

              assert-detect identity $ contains? (range 10) 6
              assert-detect not $ contains? (range 10) 16

              assert-detect identity $ &list:contains? (range 4) 3
              assert-detect not $ &list:contains? (range 4) 4
              assert-detect not $ &list:contains? (range 4) -1

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
                  range 10
                  fn (x) (> x 5)

              assert= 6
                find-index
                  range 10
                  fn (x) (> x 5)

        |test-groups $ quote
          defn test-groups ()

            assert=
              group-by
                range 10
                \ .rem % 3
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
              section-by (range 10) 2
              []
                [] 0 1
                [] 2 3
                [] 4 5
                [] 6 7
                [] 8 9
            assert=
              section-by (range 10) 3
              []
                [] 0 1 2
                [] 3 4 5
                [] 6 7 8
                [] 9
            assert=
              section-by ([]) 2
              []

        |test-comma $ quote
          assert=
            [] 1 2 3 4
            [,] 1 , 2 , 3 , 4

        |test-every $ quote
          defn test-every ()
            let
                data $ [] 1 2 3 4
              assert-detect not $ every? data
                fn (x) (&> x 1)
              assert-detect identity $ every? data
                fn (x) (&> x 0)
              assert-detect identity $ any? data
                fn (x) (&> x 3)
              assert-detect not $ any? data
                fn (x) (&> x 4)

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
              reduce ([] 3 4 5) 2 +
              , 14

        |test-apply $ quote
          defn test-apply ()
            assert= 10 $ apply + $ [] 1 2 3 4
            assert= 10 $ + & $ [] 1 2 3 4

        |test-join $ quote
          fn ()
            assert= |1-2-3-4 $ join-str ([] 1 2 3 4) |-
            assert= |1-2-3-4 $ .join-str ([] 1 2 3 4) |-
            assert= | $ join-str ([]) |-
            assert=
              [] 1 10 2 10 3 10 4
              join ([] 1 2 3 4) 10
            assert= ([]) $ join ([]) 10

        |test-repeat $ quote
          fn ()
            assert=
              repeat :a 5
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

            inside-eval:
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

            inside-eval:
              echo $ format-to-lisp $ macroexpand $ quote
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

        |test-alias $ quote
          fn ()
            log-title "|Testing alias"
            assert= (' 1 2 3) ([] 1 2 3)

        |test-methods $ quote
          fn ()
            log-title "|Testing list methods"

            assert= true
              .any? ([] 1 2 3 4)
                fn (x) (> x 3)
            assert= false
              .any? ([] 1 2 3 4)
                fn (x) (> x 4)

            assert=
              [] 1 2
              .add ([] 1) 2
            assert=
              [] 1 2
              .append ([] 1) 2
            assert=
              [] 1 3
              .assoc ([] 1 2) 1 3

            assert=
              [] 1 3 2
              .assoc-after ([] 1 2) 0 3
            assert=
              [] 1 2 3
              .assoc-after ([] 1 2) 1 3

            assert=
              [] 3 1 2
              .assoc-before ([] 1 2) 0 3
            assert=
              [] 1 3 2
              .assoc-before ([] 1 2) 1 3

            assert=
              [] 1 2
              .butlast ([] 1 2 3)

            assert=
              [] 1 2 3 4
              .concat ([] 1 2) ([] 3 4)

            assert= true
              .contains? ([] :a :b :c) 1
            assert= false
              .contains? ([] :a :b :c) 3
            assert= true
              .has-index? ([] :a :b :c) 1

            assert= true
              .includes? ([] :a :b :c) :a
            assert= false
              .includes? ([] :a :b :c) 3


            assert= 3
              .count $ [] 1 2 3

            assert=
              [] 2 3 4
              .drop ([] 1 2 3 4) 1

            assert=
              []
              .empty $ [] 1 2 3
            assert= true
              .empty? $ []
            assert= false
              .empty? $ [] 1 2 3

            assert=
              [] 3 4
              .filter ([] 1 2 3 4)
                fn (x) (> x 2)

            assert=
              [] 1 2
              .filter-not ([] 1 2 3 4)
                fn (x) (> x 2)

            assert= 0
              .find-index ([] :a :b :c) $ fn (x) (= x :a)
            assert= nil
              .find-index ([] :a :b :c) $ fn (x) (= x :d)

            assert= 10
              .foldl ([] 1 2 3 4) 0 +

            assert=
              {}
                1 1
                2 2
                3 3
              .frequencies ([] 1 2 2 3 3 3)

            assert= :b
              .get ([] :a :b :c :d) 1

            assert= :c
              .get-in ([] :a ([] :b ([] :c))) ([] 1 1 0)
            assert= nil
              .get-in ([] :a ([] :b ([] :c))) ([] 1 1 1)
            assert=
              {}
                1 $ [] 1 4
                2 $ [] 2
                0 $ [] 3
              .group-by ([] 1 2 3 4) $ fn (x)
                .rem x 3

            assert= 0
              .index-of ([] :a :b :c :d) :a
            assert= nil
              .index-of ([] :a :b :c :d) :e
            assert=
              [] :a 1 :b 2 :c 3 :d 4
              .interleave ([] :a :b :c :d) ([] 1 2 3 4)
            assert=
              [] 1 :sep 2 :sep 3 :sep 4 :sep 5
              .join ([] 1 2 3 4 5) :sep

            assert=
              [] 4 5 6
              .map ([] 1 2 3) $ fn (x) (+ x 3)
            assert=
              [] 2 3 4
              .map ([] 1 2 3) .inc
            assert=
              []
                [] 0 :a
                [] 1 :b
                [] 2 :c
              .map-indexed ([] :a :b :c) $ fn (idx x)
                [] idx x

            assert= 4
              .max ([] 1 2 3 4)
            assert= 1
              .min ([] 1 2 3 4)

            assert= :b
              .nth ([] :a :b :c :d) 1
            assert= nil
              .nth ([] :a :b :c :d) 5

            assert=
              [] 4 3 2 1
              .sort-by ([] 1 2 3 4) negate

            assert=
              {}
                :a 1
                :b 2
              .pairs-map $ []
                [] :a 1
                [] :b 2

            assert=
              [] 5 1 2 3 4
              .prepend ([] 1 2 3 4) 5

            assert= 10
              .reduce ([] 1 2 3 4) 0 +
            assert=
              [] 4 3 2 1
              .reverse $ [] 1 2 3 4
            assert=
              []
                [] 1 2
                [] 3 4
                [] 5
              .section-by ([] 1 2 3 4 5) 2

            assert=
              [] :b :c :d
              .slice ([] :a :b :c :d) 1 4

            assert=
              [] 1 2 3 4 5
              .sort ([] 1 4 2 5 3) $ fn (x y)
                - x y

            assert=
              [] 1 2 3 4
              .sort-by ([] 1 2 3 4) inc

            assert=
              []
                {} (:v :a) (:n 1)
                {} (:v :c) (:n 2)
                {} (:v :b) (:n 3)
              .sort-by
                []
                  {} (:v :a) (:n 1)
                  {} (:v :b) (:n 3)
                  {} (:v :c) (:n 2)
                , :n

            assert=
              [] :a :b
              .take ([] :a :b :c :d) 2

            assert=
              &{} :a 1 :b 2 :c 3
              .zipmap ([] :a :b :c) ([] 1 2 3)

            assert= 1
              .first $ [] 1 2 3 4
            assert=
              [] 2 3 4
              .rest $ [] 1 2 3 4
            assert=
              [] :a :b
              .dissoc ([] :a :b :c) 2

            assert=
              [] 1 2 3
              .distinct ([] 1 2 3 1 2)

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

            test-alias

            test-doseq
            test-let[]

            test-methods

            do true

      :proc $ quote ()
      :configs $ {} (:extension nil)

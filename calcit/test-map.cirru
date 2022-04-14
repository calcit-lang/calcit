
{} (:package |test-map)
  :configs $ {} (:init-fn |test-map.main/main!) (:reload-fn |test-map.main/reload!)
    :modules $ [] |./util.cirru
  :files $ {}
    |test-map.main $ {}
      :ns $ quote
        ns test-map.main $ :require
          [] util.core :refer $ [] log-title inside-eval: inside-js:
      :defs $ {}

        |test-maps $ quote
          defn test-maps ()
            assert= 2 $ count $ {} (:a 1) (:b 2)
            let
                dict $ merge
                  {} (:a 1) (:b 2)
                  {} (:c 3) (:d 5)
              assert= 4 $ count dict
              assert-detect identity (contains? dict :a)
              assert-detect not (contains? dict :a2)

              assert-detect identity (includes? dict 2)
              assert-detect not (includes? dict :a)

              ; println $ keys dict
              assert=
                keys dict
                #{} :c :a :b :d

              assert=
                vals $ {} (:a 1) (:b 2) (:c 2)
                #{} 2 1

              assert= (assoc dict :h 10) $ {}
                :a 1
                :b 2
                :c 3
                :d 5
                :h 10
              assert=
                assoc (&{} :a 1 :b 2) :a 3
                &{} :a 3 :b 2
              assert=
                assoc (&{} :a 1 :b 2) :b 3
                &{} :a 1 :b 3
              assert=
                assoc (&{} :a 1 :b 2) :c 3
                &{} :a 1 :b 2 :c 3
              assert=
                assoc (&{} :a 1) :b 2 :c 3
                &{} :a 1 :b 2 :c 3

              inside-js:
                &let
                  data $ &{} :a 1
                  .!turnMap data
                  assert=
                    assoc data :b 2 :c 3
                    &{} :a 1 :b 2 :c 3

              assert=
                dissoc dict :a
                {,} :b 2 , :c 3 , :d 5
              assert= dict (dissoc dict :h)
              assert=
                dissoc dict :a :b :c
                &{} :d 5

              assert=
                merge
                  {}
                    :a 1
                    :b 2
                  {}
                    :c 3
                  {}
                    :d 4
                {} (:a 1) (:b 2) (:c 3) (:d 4)

              assert=
                merge
                  {,} :a 1 , :b 2 , :c 3
                  {,} :a nil , :b 12
                  {,} :c nil , :d 14
                {,} :a nil , :b 12 , :c nil , :d 14

              assert=
                merge-non-nil
                  {,} :a 1 , :b 2 , :c 3
                  {,} :a nil , :b 12
                  {,} :c nil , :d 14
                {,} :a 1 , :b 12 , :c 3 , :d 14

              assert=
                merge
                  {} (:a true) (:b false) (:c true) (:d false)
                  {} (:a false) (:b false) (:c true) (:d true)
                {} (:a false) (:b false) (:c true) (:d true)

              assert=
                merge ({} (:a 1)) nil
                {} (:a 1)

        |test-pairs $ quote
          fn ()

            assert=
              pairs-map $ []
                [] :a 1
                [] :b 2
              {} (:a 1) (:b 2)

            assert=
              zipmap
                [] :a :b :c
                [] 1 2 3
              {}
                :a 1
                :b 2
                :c 3

            assert=
              to-pairs $ {}
                :a 1
                :b 2
              #{}
                [] :a 1
                [] :b 2

            assert=
              map-kv
                {} (:a 1) (:b 2)
                fn (k v) ([] k (+ v 1))
              {} (:a 2) (:b 3)

            assert=
              filter
                {} (:a 1) (:b 2) (:c 3) (:d 4)
                fn (pair) $ let[] (k v) pair
                  &> v 2
              {} (:c 3) (:d 4)

            assert=
              .filter
                {} (:a 1) (:b 2) (:c 3) (:d 4)
                fn (pair) $ let[] (k v) pair
                  &> v 2
              {} (:c 3) (:d 4)

            assert=
              .filter-kv
                {} (:a 1) (:b 2) (:c 3) (:d 4)
                fn (k v)
                  &> v 2
              {} (:c 3) (:d 4)

        |test-native-map-syntax $ quote
          defn test-native-map-syntax ()

            inside-eval:
              assert=
                macroexpand $ quote $ {} (:a 1)
                quote $ &{} :a 1

        |test-map-comma $ quote
          fn ()
            log-title "|Testing {,}"

            inside-eval:
              assert=
                macroexpand $ quote $ {,} :a 1 , :b 2 , :c 3
                quote $ pairs-map $ section-by ([] :a 1 :b 2 :c 3) 2
            assert=
              {,} :a 1 , :b 2 , :c 3
              {} (:a 1) (:b 2) (:c 3)

        |test-keys $ quote
          fn ()
            log-title "|Testing keys"

            assert=
              keys-non-nil $ {}
                :a 1
                :b 2
              #{} :a :b

            assert=
              keys-non-nil $ {}
                :a 1
                :b 2
                :c nil
              #{} :a :b

        |test-get $ quote
          fn ()
            log-title "|Testing get"

            assert= nil $ get (&{}) :a
            assert= nil $ get-in (&{}) $ [] :a :b

            assert= nil $ get nil :a

            &let
              m $ &{} :a 1 :b 2 :c 3 :d 4
              assert= (first m) (first m)
              assert= (rest m) (rest m)
              assert= 3 (count $ rest m)

              assert= 10
                foldl m 0 $ fn (acc pair)
                  let[] (k v) pair
                    &+ acc v

        |test-select $ quote
          fn ()
            log-title "|Testing select"
            assert=
              select-keys ({} (:a 1) (:b 2) (:c 3)) ([] :a :b)
              {} (:a 1) (:b 2)
            assert=
              select-keys ({} (:a 1) (:b 2) (:c 3)) ([] :d)
              {} (:d nil)

            assert=
              unselect-keys ({} (:a 1) (:b 2) (:c 3)) ([] :a :b)
              {} (:c 3)
            assert=
              unselect-keys ({} (:a 1) (:b 2) (:c 3)) ([] :c :d)
              {} (:a 1) (:b 2)

        |test-methods $ quote
          fn ()
            log-title "|Testing map methods"

            assert=
              &{} :a 1 :b 2
              .add (&{} :a 1) ([] :b 2)

            assert=
              &{} :a 1 :b 2
              .assoc (&{} :a 1) :b 2

            assert= true
              .contains? (&{} :a 1) :a
            assert= false
              .contains? (&{} :a 1) :b

            assert= 2
              .count $ {} (:a 1) (:b 2)

            assert=
              &{} :a 1
              .dissoc (&{} :a 1 :b 2) :b
            assert=
              &{} :a 1
              .dissoc (&{} :a 1 :b 2 :c 3) :b :c

            assert=
              &{}
              .empty $ &{} :a 1 :b 2

            assert= false
              .empty? $ &{} :a 1 :b 2
            assert= true
              .empty? $ &{}

            assert= 1
              .get (&{} :a 1) :a
            assert= nil
              .get (&{} :a 1) :b

            assert= 2
              .get-in
                {}
                  :a $ {}
                    :b 2
                [] :a :b

            assert= nil
              .get-in (&{})
                [] :a :b

            assert= true
              .includes? (&{} :a 1 :b 2) 1
            assert= false
              .includes? (&{} :a 1 :b 2) 3

            assert=
              #{} :a :b
              .keys $ &{} :a 1 :b 2
            assert=
              #{} :a :b
              keys-non-nil $ &{} :a 1 :b 2 :c nil

            assert=
              {} (:a 11) (:b 12)
              .map (&{} :a 1 :b 2) $ fn (entry)
                []
                  first entry
                  + 10 $ last entry

            ; "not so stable, :bbbb is rare so it could be larger"
            assert=
              []
                [] :a 11
                [] :bbbb 12
              .sort-by
                .map-list (&{} :a 1 :bbbb 2) $ fn (entry)
                  []
                    first entry
                    + 10 $ last entry
                , first

            assert=
              {} (:a 11)
              .map-kv ({} (:a 1)) $ fn (k v)
                [] k (+ v 10)

            assert=
              {} (:a 11) (:b 12)
              .map-kv ({} (:a 1) (:b 2) (:c 13))
                fn (k v)
                  if (< v 10)
                    [] k (+ v 10)
                    , nil

            assert=
              &{} :a 1 :b 2
              .merge
                &{} :a 1
                &{} :b 2

            assert=
              &{} :a 1 :b 2
              select-keys
                &{} :a 1 :b 2 :c 3
                [] :a :b

            assert=
              [] ([] :a 1)
              .to-list $ {} (:a 1)

            assert= 2
              .count $ .to-list $ {}
                :a 1
                :b 2

            assert= 2
              .count $ .to-pairs $ {}
                :a 1
                :b 2

            assert=
              &{} :a 1 :b 2
              unselect-keys
                &{} :a 1 :b 2 :c 3
                [] :c

            assert=
              #{} 1 2 3
              .values $ &{} :a 1 :b 2 :c 3

            assert= true
              list? $ .first $ &{} :a 1 :b 2 :c 3
            assert= 2
              count $ .first $ &{} :a 1 :b 2 :c 3

            assert= 2
              .count $ .rest $ &{} :a 1 :b 2 :c 3

            assert=
              &{} :c 3
              .diff-new (&{} :a 1 :b 2 :c 3) (&{} :a 2 :b 3)
            assert=
              #{} :c
              .diff-keys (&{} :a 1 :b 2 :c 3) (&{} :a 2 :b 3)
            assert=
              #{} :a :b
              .common-keys (&{} :a 1 :b 2 :c 3) (&{} :a 2 :b 3)

            assert=
              &{} :a 1
              .to-map (&{} :a 1)

        |test-diff $ quote
          fn ()
            log-title "|Testing diff"

            assert=
              &map:diff-new
                &{} :a 1 :b 2
                &{} :a 2 :b 3
              &{}

            assert=
              &map:diff-new
                &{} :a 1 :b 2 :c 3
                &{} :a 2 :b 3
              &{} :c 3

            assert=
              &map:diff-new
                &{} :a 1 :b 2
                &{} :a 2 :b 3 :c 4
              &{}

            assert=
              &map:diff-keys
                &{} :a 1 :b 2
                &{} :a 2
              #{} :b

            assert=
              &map:diff-keys
                &{} :a 1 :b 2
                &{} :a 2 :c 3
              #{} :b

            assert=
              &map:common-keys
                &{} :a 1 :b 2
                &{} :a 2 :c 3
              #{} :a

        |main! $ quote

          defn main! ()

            log-title "|Testing maps"
            test-maps

            log-title "|Testing map pairs"
            test-pairs

            log-title "|Testing map syntax"
            test-native-map-syntax

            test-map-comma

            test-keys

            test-get

            test-select

            test-methods

            test-diff

            do true

      :proc $ quote ()
      :configs $ {} (:extension nil)


{} (:package |test-map)
  :configs $ {} (:init-fn |test-map.main/main!) (:reload-fn |test-map.main/reload!)
    :modules $ [] |./util.cirru
  :files $ {}
    |test-map.main $ {}
      :ns $ quote
        ns test-map.main $ :require
          [] util.core :refer $ [] log-title inside-nim:
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

              ; echo $ keys dict
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
                dissoc dict :a
                {,} :b 2 , :c 3 , :d 5
              assert= dict (dissoc dict :h)
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
                merge-non-nil
                  {,} :a 1 , :b 2 , :c 3
                  {,} :a nil , :b 12
                  {,} :c nil , :d 14
                {,} :a 1 , :b 12 , :c 3 , :d 14

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
                fn (k v) ([] k (+ v 1))
                {} (:a 1) (:b 2)
              #{}
                [] :a 2
                [] :b 3

        |test-native-map-syntax $ quote
          defn test-native-map-syntax ()

            inside-nim:
              assert=
                macroexpand $ quote $ {} (:a 1)
                quote $ &{} :a 1

        |test-map-comma $ quote
          fn ()
            log-title "|Testing {,}"

            inside-nim:
              assert=
                macroexpand $ quote $ {,} :a 1 , :b 2 , :c 3
                quote $ pairs-map $ section-by 2 $ [] :a 1 :b 2 :c 3
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

            do true

      :proc $ quote ()
      :configs $ {} (:extension nil)

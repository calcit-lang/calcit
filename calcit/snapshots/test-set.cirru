
{} (:package |test-set)
  :configs $ {} (:init-fn |test-set.main/main!) (:reload-fn |test-set.main/reload!)
  :files $ {}
    |test-set.main $ {}
      :ns $ quote
        ns test-set.main $ :require
      :defs $ {}

        |log-title $ quote
          defn log-title (title)
            echo
            echo title
            echo

        |test-set $ quote
          defn test-set ()
            assert= 4 $ count $ #{} 1 2 3 4
            assert-detect identity $ includes? (#{} 1 2 3) 2
            assert= false $ includes? (#{} 1 2 3) 4
            assert= (#{} 1 2 3) (#{} 2 3 1)
            assert= (include (#{} 1 2 3) 4) (#{} 1 2 3 4)
            assert= (include (#{} 1 2) 3 4) (#{} 1 2 3 4)
            assert= (exclude (#{} 1 2 3) 1) (#{} 2 3)
            assert= (exclude (#{} 1 2 3) 1 2) (#{} 3)

            assert=
              difference (#{} 1 2 3) (#{} 1) (#{} 2)
              #{} 3
            assert=
              union (#{} 1) (#{} 2) (#{} 3)
              #{} 1 2 3
            assert=
              intersection (#{} 1 2 3) (#{} 2 3 4) (#{} 3 4 5)
              #{} 3

            &let
              v $ set->list $ #{} 1 2 3
              assert-detect list? v
              assert= 3 $ count v

            assert=
              map
                #{} 1 2 3
                fn (x) (inc x)
              #{} 2 3 4

            assert-detect identity
              every?
                #{} 1 2 3
                \ > % 0

            assert= (#{} 1 2 3) (#{} 1 2 (+ 1 2))
            assert-detect not $ = (#{} 1 2 3) (#{} 2 3 4)

        |test-methods $ quote
          fn ()
            assert= 3
              .count $ #{} 1 2 3

        |main! $ quote
          defn main! ()
            log-title "|Testing set"
            test-set

            test-methods

            do true

      :proc $ quote ()
      :configs $ {} (:extension nil)

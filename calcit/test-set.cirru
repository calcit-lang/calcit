
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
              v $ .to-list $ #{} 1 2 3
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

            assert=
              #{} 1 2 3 4
              &list:to-set $ [] 1 2 3 4

            assert=
              #{} 1
              &list:to-set $ [] 1 1 1 1
            assert=
              #{} 1
              .to-set $ [] 1 1 1 1

        |test-methods $ quote
          fn ()
            assert=
              #{} 1 2 3
              .add (#{} 1 2) 3
            assert= 3
              .count $ #{} 1 2 3
            assert=
              #{} 3
              .difference (#{} 1 2 3) (#{} 1 2)
            assert=
              #{} :c
              .difference (#{} :a :b :c) (#{} :a :b)
            assert=
              #{} ([] 1 3)
              .difference (#{} ([] 1 2) ([] 1 3)) (#{} ([] 1 2))
            assert=
              #{} 1 2
              .exclude (#{} 1 2 3) 3
            assert= (#{}) $ .empty $ #{} 1 2 3
            assert= false $ .empty? $ #{} 1 2 3
            assert= true $ .empty? $ #{}
            assert=
              #{} 1 2 3
              .include (#{} 1 2) 3
            assert= true $ .includes? (#{} 1 2 3) 1
            assert= false $ .includes? (#{} 1 2 3) 4

            assert=
              #{} 2
              .intersection (#{} 1 2) (#{} 2 3)

            assert= true
              list? $ .to-list $ #{} 1 2 3
            assert= 3
              count $ .to-list $ #{} 1 2 3

            assert=
              #{} 1 2 3
              .union (#{} 1 2) (#{} 2 3)

            assert= true
              some? $ .first $ #{} 1 2 3
            assert= 2
              count $ .rest $ #{} 1 2 3

            assert=
              #{} 1 3 5
              .to-set $ #{} 1 3 5

            assert=
              #{} 7 9
              .filter (#{} 1 3 5 7 9)
                fn (x) (&> x 5)

            assert= 4
              .max $ #{} 1 2 3 4

            assert= 1
              .min $ #{} 1 2 3 4

        |main! $ quote
          defn main! ()
            log-title "|Testing set"
            test-set

            test-methods

            do true

      :proc $ quote ()
      :configs $ {} (:extension nil)

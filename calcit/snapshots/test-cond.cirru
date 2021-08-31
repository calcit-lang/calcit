
{} (:package |test-cond)
  :configs $ {} (:init-fn |test-cond.main/main!) (:reload-fn |test-cond.main/reload!)
    :modules $ [] |./util.cirru
  :files $ {}
    |test-cond.main $ {}
      :ns $ quote
        ns test-cond.main $ :require
          [] util.core :refer $ [] inside-eval:
      :defs $ {}

        |test-cond $ quote
          defn test-cond ()
            let
                compare-x $ fn (x)
                  cond
                    (&> x 10) "|>10"
                    (&> x 5) "|>5"
                    true "|<=5"
              assert= (compare-x 11) "|>10"
              assert= (compare-x 10) "|>5"
              assert= (compare-x 6) "|>5"
              assert= (compare-x 4) "|<=5"

        |test-case $ quote
          defn test-case ()
            let
                detect-x $ fn (x)
                  case x
                    1 "|one"
                    2 "|two"
                    x "|else"
              assert= (detect-x 1) "|one"
              assert= (detect-x 2) "|two"
              assert= (detect-x 3) "|else"

            inside-eval:
              assert=
                macroexpand $ quote
                  case-default x |nothing
                    1 |one
                    2 |two
                quote $ &let (v__2 x)
                  &case v__2 |nothing (1 |one) (2 |two)

            &let
              detect-x $ fn (x)
                case-default x |nothing
                  1 |one
                  2 |two
              assert= (detect-x 0) |nothing
              assert= (detect-x 1) |one
              assert= (detect-x 2) |two

        |test-or $ quote
          fn ()
            log-title "|Testing or"
            assert= (or 1) 1
            assert= (or nil) nil
            assert= (or 1 nil) 1
            assert= (or nil 1) 1
            assert= (or nil nil 1) 1
            assert= (or nil nil nil) nil

            assert= (or (> 10 9) (> 10 8)) true
            assert= (or (> 10 11) (> 10 8)) true
            assert= (or (> 10 9) (> 10 11)) true
            assert= (or (> 10 12) (> 10 11)) false

        |test-and $ quote
          fn ()
            log-title "|Testing and"

            assert= (and 1) 1
            assert= (and nil) false
            assert= (and 1 nil) false
            assert= (and nil 1) false
            assert= (and 1 1) 1

            assert= (and 1 1 1) 1
            assert= (and 1 1 nil) false
            assert= (and nil 1 1) false

            assert= (and (> 10 9) (> 10 8)) true
            assert= (and (> 10 11) (> 10 8)) false
            assert= (and (> 10 9) (> 10 11)) false

        |test-either $ quote
          fn ()
            log-title "|Testing either"

            assert= 1
              either nil 1
            assert= 1
              either 1 nil
            assert= nil
              either nil nil
            assert= 1
              either nil nil 1

            assert= 1
              either (do nil) (do 1) (do nil)

        |log-title $ quote
          defn log-title (title)
            echo
            echo title
            echo

        |test-key-match $ quote
          fn ()
            log-title "|Testing key-match"
            &let
              match-ab $ fn (data)
                key-match data
                  (:a x) (' "|pattern a:" x)
                  (:b x y) (' "|pattern b:" x y)
                  _ (' "|no match")
              assert=
                match-ab (' :a 1 2)
                [] "|pattern a:" 1
              assert=
                match-ab (' :b 1 2)
                [] "|pattern b:" 1 2
              assert=
                match-ab (' :c 1 2)
                [] "|no match"

        |main! $ quote
          defn main! ()
            log-title "|Testing cond"
            test-cond

            test-or
            test-and
            test-either

            test-case

            test-key-match

            , true

      :proc $ quote ()
      :configs $ {} (:extension nil)

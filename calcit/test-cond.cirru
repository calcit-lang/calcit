
{} (:package |test-cond)
  :configs $ {} (:init-fn |test-cond.main/main!) (:reload-fn |test-cond.main/reload!)
    :modules $ [] |./util.cirru
  :files $ {}
    |test-cond.main $ %{} :FileEntry
      :defs $ {}
        |log-title $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn log-title (title) (println) (println title) (println)
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! () (log-title "|Testing cond") (test-when) (test-cond) (test-or) (test-and) (test-either) (test-case) (test-tag-match) (test-field-match) true
        |test-and $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing and")
              assert= (and 1) 1
              assert= (and nil) false
              assert= (and 1 nil) false
              assert= (and nil 1) false
              assert= (and 1 1) 1
              assert= (and 1 1 1) 1
              assert= (and 1 1 nil) false
              assert= (and nil 1 1) false
              assert=
                and (> 10 9) (> 10 8)
                , true
              assert=
                and (> 10 11) (> 10 8)
                , false
              assert=
                and (> 10 9) (> 10 11)
                , false
        |test-case $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-case ()
              let
                  detect-x $ fn (x)
                    case x (1 |one) (2 |two) (x |else)
                assert= (detect-x 1) |one
                assert= (detect-x 2) |two
                assert= (detect-x 3) |else
              inside-eval: $ assert=
                macroexpand $ quote
                  case-default x |nothing (1 |one) (2 |two)
                quote $ &let (v__2 x)
                  &case v__2 |nothing (1 |one) (2 |two)
              &let
                detect-x $ fn (x)
                  case-default x |nothing (1 |one) (2 |two)
                assert= (detect-x 0) |nothing
                assert= (detect-x 1) |one
                assert= (detect-x 2) |two
        |test-cond $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-cond () $ let
                compare-x $ fn (x)
                  cond
                      &> x 10
                      , |>10
                    (&> x 5) |>5
                    true |<=5
              assert= (compare-x 11) |>10
              assert= (compare-x 10) |>5
              assert= (compare-x 6) |>5
              assert= (compare-x 4) |<=5
        |test-either $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing either")
              assert= 1 $ either nil 1
              assert= 1 $ either 1 nil
              assert= nil $ either nil nil
              assert= 1 $ either nil nil 1
              assert= 1 $ either (do nil) (do 1) (do nil)
        |test-field-match $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing field-match")
              &let
                match-ab $ fn (data)
                  field-match data
                    :a a $ [] :a (:a a)
                    :b b $ [] :b (:b b)
                    _ :other
                assert=
                  match-ab $ &{} :tag :a :a 1
                  [] :a 1
                assert=
                  match-ab $ &{} :tag :b :b 2
                  [] :b 2
                assert= :other $ match-ab (&{} :tag :c)
        |test-or $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing or")
              assert= (or 1) 1
              assert= (or nil) nil
              assert= (or 1 nil) 1
              assert= (or nil 1) 1
              assert= (or nil nil 1) 1
              assert= (or nil nil nil) nil
              assert=
                or (> 10 9) (> 10 8)
                , true
              assert=
                or (> 10 11) (> 10 8)
                , true
              assert=
                or (> 10 9) (> 10 11)
                , true
              assert=
                or (> 10 12) (> 10 11)
                , false
        |test-tag-match $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing tag-match")

              ; println |EXPANDED $ format-to-cirru $ macroexpand-all $ quote $ tag-match (:: :a 1)
                (:a x) 1
                (:a x y) 2
                _ :none

              &let
                match-ab $ fn (data)
                  tag-match data
                      :a x
                      ' "|pattern a:" x
                    (:b x y) (' "|pattern b:" x y)
                    _ $ ' "|no match"
                assert=
                  match-ab $ :: :a 1
                  [] "|pattern a:" 1
                assert=
                  match-ab $ :: :b 1 2
                  [] "|pattern b:" 1 2
                assert=
                  match-ab $ :: :c 1 2
                  [] "|no match"
                assert=
                  match-ab $ :: :a 1 2
                  [] "|no match"
                assert=
                  match-ab $ :: :b 1 2
                  [] "|pattern b:" 1 2
                assert=
                  match-ab $ :: :c 1 2
                  [] "|no match"
        |test-when $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing when")
              assert= 1 $ when true 1
              assert= 1 $ when true 2 1
              assert= 1 $ when-not false 1
              assert= 1 $ when-not false 2 1
      :ns $ %{} :CodeEntry (:doc |)
        :code $ quote
          ns test-cond.main $ :require
            [] util.core :refer $ [] inside-eval:

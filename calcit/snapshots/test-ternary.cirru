
{} (:package |test-ternary)
  :configs $ {} (:init-fn |test-ternary.main/main!) (:reload-fn |test-ternary.main/reload!)
  :files $ {}
    |test-ternary.main $ {}
      :ns $ quote
        ns test-ternary.main $ :require
      :defs $ {}

        |log-title $ quote
          defn log-title (title)
            echo
            echo title
            echo

        |test-ternary $ quote
          fn ()
            ; "TODO ternary not in Rust yet"
            log-title "|Testing ternary"

            assert= &1 &1
            assert= &1.3 &1.3
            assert= (&+ &1 &1) &19
            assert= (+ &1 &1 &1) &15
            assert= (+ &1 &1 &1 &1) &11
            assert= (&- &44 &6) &466
            assert= (dbt->point &33) ([] 4 0)
            assert= (dbt->point &66) ([] -4 4)
            assert= (dual-balanced-ternary 4 4) &88

            assert= (round &3.333) &3
            assert= (round &3.333 0) &3
            assert= (round &3.333 1) &3.3
            assert= (round &3.333 2) &3.33

        |test-digits $ quote
          fn ()

            log-title "|Testing dbt-digits"

            echo $ dbt-digits &34.56

        |main! $ quote
          defn main! ()
            test-ternary

            test-digits

            do true

      :proc $ quote ()
      :configs $ {} (:extension nil)

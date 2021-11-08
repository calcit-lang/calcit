
{} (:package |test-recursion)
  :configs $ {} (:init-fn |test-recursion.main/main!) (:reload-fn |test-recursion.main/reload!)
  :files $ {}
    |test-recursion.main $ {}
      :ns $ quote
        ns test-recursion.main $ :require
      :defs $ {}

        |log-title $ quote
          defn log-title (title)
            echo
            echo title
            echo

        |hole-series $ quote
          defn hole-series (x)
            if (&<= x 0) (raise "\"unexpected small number")
              if (&= x 1) (, 0)
                if (&= x 2) (, 1)
                  let
                      extra $ .rem x 3
                    if (&= extra 0)
                      let
                          unit $ &/ x 3
                        &* 3 $ hole-series unit
                      if (&= extra 1)
                        let
                            unit $ &/ (&- x 1) (, 3)
                          &+ (&* 2 $ hole-series unit) (hole-series $ &+ unit 1)
                        let
                            unit $ &/ (&- x 2) (, 3)
                          &+
                            &* 2 $ hole-series (&+ unit 1)
                            hole-series unit

        |test-hole-series $ quote
          defn test-hole-series ()
            assert "|hole series numbers" $ = (map (range 1 20) hole-series)
              [] 0 1 0 1 2 3 2 1 0 1 2 3 4 5 6 7 8 9 8

        |*count-effects $ quote
          defatom *count-effects 0

        |test-loop $ quote
          fn ()
            assert= 55 $ apply
              defn add-range (acc from to)
                if (> from to) acc
                  recur (&+ acc from) (inc from) to
              [] 0 1 10
            assert= 55 $ loop
                acc 0
                from 1
                to 10
              if (> from to) acc
                recur (&+ acc from) (inc from) to

            reset! *count-effects 0
            loop ((x 3))
              if (> x 0)
                do
                  swap! *count-effects + x
                  recur (dec x)
            assert= 6 @*count-effects

        |main! $ quote
          defn main! ()
            log-title "|Testing hole series"
            test-hole-series

            ; set-trace-fn! |app.main |hole-series
            ; echo (hole-series 100)

            log-title "|Testing loop"
            test-loop

            do true

      :proc $ quote ()
      :configs $ {} (:extension nil)

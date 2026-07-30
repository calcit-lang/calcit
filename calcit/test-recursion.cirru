
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |test-recursion) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:init-fn |test-recursion.main/main!) (:mode :native) (:reload-fn |test-recursion.main/reload!)
      :modules $ []
      :type-slots $ {}
  :files $ {}
    |test-recursion.main $ %{} :FileEntry
      :defs $ {}
        |*count-effects $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote (defatom *count-effects 0)
          :examples $ []
        |hole-series $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn hole-series (x) (assert-type x :number)
              if (&<= x 0) (raise "|unexpected small number")
                if (&= x 1) 0 $ if (&= x 2) 1
                  let
                      extra $ .rem x 3
                    if (&= extra 0)
                      let
                          unit $ &/ x 3
                        &* 3 $ hole-series unit
                      if (&= extra 1)
                        let
                            unit $ &/ (&- x 1) 3
                          &+
                            &* 2 $ hole-series unit
                            hole-series $ &+ unit 1
                        let
                            unit $ &/ (&- x 2) 3
                          &+
                            &* 2 $ hole-series (&+ unit 1)
                            hole-series unit
          :examples $ []
          :schema $ :: :fn
            {} (:return :number)
              :args $ [] :number
        |log-title $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn log-title (title) (println) (println title) (println)
          :examples $ []
          :schema $ :: :fn
            {} (:return :dynamic)
              :args $ [] :dynamic
        |main! $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defn main! () (log-title "|Testing hole series") (test-hole-series) (; set-trace-fn! |app.main |hole-series)
              ; println $ hole-series 100
              log-title "|Testing loop"
              test-loop
              do true
          :examples $ []
        |test-hole-series $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defn test-hole-series () $ assert "|hole series numbers"
              =
                map (range 1 20) hole-series
                [] 0 1 0 1 2 3 2 1 0 1 2 3 4 5 6 7 8 9 8
          :examples $ []
        |test-loop $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            fn ()
              assert= 55 $ apply
                defn add-range (acc from to)
                  if (> from to) acc $ recur (&+ acc from) (inc from) to
                [] 0 1 10
              assert= 55 $ loop
                  acc 0
                  from 1
                  to 10
                if (> from to) acc $ recur (&+ acc from) (inc from) to
              reset! *count-effects 0
              loop
                  x 3
                if (> x 0)
                  do (swap! *count-effects + x)
                    recur $ dec x
              assert= 6 @*count-effects
          :examples $ []
      :ns $ %{} :NsEntry (:doc |)
        :code $ quote
          ns test-recursion.main $ :require

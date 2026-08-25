
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --full` first. Manual edits must follow format and schema conventions, then run `calcit edit format`.") (:package |app)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
    :prime $ {} (:description |) (:init-fn 'app.main/try-prime) (:mode :native) (:reload-fn 'app.main/try-prime)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    |app.main $ %{} 'FileEntry
      :defs $ {}
        |bench-rem-direct! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn bench-rem-direct! () $ println (loop-rem-direct 500000 0)
          :examples $ []
          :schema $ :: 'Dynamic
        |bench-rem-dynamic! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn bench-rem-dynamic! () $ println (loop-rem-dynamic 500000 0)
          :examples $ []
          :schema $ :: 'Dynamic
        |bench-rem-typed! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn bench-rem-typed! () $ println (loop-rem-typed 500000 0)
          :examples $ []
          :schema $ :: 'Dynamic
        |fibo $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn fibo (x)
              if (< x 2) 1 $ +
                fibo $ - x 1
                fibo $ - x 2
          :examples $ []
          :schema $ :: 'Dynamic
        |loop-rem-direct $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn loop-rem-direct (n acc)
              hint-fn $ {}
                :args $ [] 'Number 'Number
                :return 'Number
              if (&< n 1) acc $ recur (&- n 1)
                &+ acc $ &number:rem n 97
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'Number 'Number
        |loop-rem-dynamic $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn loop-rem-dynamic (n acc)
              hint-fn $ {}
                :args $ [] 'Number 'Number
                :return 'Number
              if (&< n 1) acc $ recur (&- n 1)
                &+ acc $ .rem (unsafe-coerce n 'Dynamic) 97
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'Number 'Number
        |loop-rem-typed $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn loop-rem-typed (n acc)
              hint-fn $ {}
                :args $ [] 'Number 'Number
                :return 'Number
              if (&< n 1) acc $ recur (&- n 1)
                &+ acc $ .rem n 97
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'Number 'Number
        |main! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn main! () (println "|Loaded program!")
              do (test-rem-methods!) (try-fibo)
          :examples $ []
          :schema $ :: 'Dynamic
        |reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn reload! () $ :: 'Unit
          :examples $ []
          :schema $ :: 'Dynamic
        |rem-direct $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn rem-direct (n divisor)
              hint-fn $ {}
                :args $ [] 'Number 'Number
                :return 'Number
              &number:rem n divisor
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'Number 'Number
        |rem-dynamic $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn rem-dynamic (n divisor)
              hint-fn $ {}
                :args $ [] 'Number 'Number
                :return 'Number
              .rem (unsafe-coerce n 'Dynamic) divisor
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'Number 'Number
        |rem-typed $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn rem-typed (n divisor)
              hint-fn $ {}
                :args $ [] 'Number 'Number
                :return 'Number
              .rem n divisor
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'Number 'Number
        |sieve-primes $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn sieve-primes (acc n limit)
              if (&> n limit) acc $ if
                every? acc $ fn (m)
                  &> (.rem n m) 0
                recur (conj acc n) (inc n) limit
                recur acc (inc n) limit
          :examples $ []
          :schema $ :: 'Dynamic
        |test-rem-methods! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn test-rem-methods! ()
              assert= 1 $ rem-typed 98 97
              assert= 1 $ rem-dynamic 98 97
              assert= 1 $ rem-direct 98 97
              assert= (loop-rem-direct 1000 0) (loop-rem-typed 1000 0)
              assert= (loop-rem-direct 1000 0) (loop-rem-dynamic 1000 0)
          :examples $ []
          :schema $ :: 'Dynamic
        |try-fibo $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn try-fibo () $ let
                n 22
              println "|fibo result:" n $ fibo n
          :examples $ []
          :schema $ :: 'Dynamic
        |try-prime $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn try-prime () $ println
              sieve-primes ([] 2 3 5 7 11 13) 17 400
          :examples $ []
          :schema $ :: 'Dynamic
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote
          ns app.main $ :require

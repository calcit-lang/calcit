
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |app
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
    :prime $ {} (:description |) (:init-fn 'app.main/try-prime) (:mode :native) (:reload-fn 'app.main/try-prime)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'app.main
    %{} 'FileEntry
      :defs $ {}
        'bench-rem-direct! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn bench-rem-direct! ()
            println $ loop-rem-direct 500000 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'fibo $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn fibo (x)
            if (< x 2) 1 $ +
              fibo $ - x 1
              fibo $ - x 2
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number
        'loop-rem-direct $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn loop-rem-direct (n acc)
            hint-fn $ {}
              :args $ [] 'Number 'Number
              :return 'Number
            if (&< n 1) acc $ recur (&- n 1)
              &+ acc $ &number:rem n 97
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number 'Number
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () (println "|Loaded program!")
            do (test-rem-methods!) (try-fibo)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'rem-direct $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn rem-direct (n divisor)
            hint-fn $ {}
              :args $ [] 'Number 'Number
              :return 'Number
            &number:rem n divisor
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number 'Number
        'sieve-primes $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn sieve-primes (acc n limit)
            if (&> n limit) acc $ if
              every? acc $ fn (m)
                &> (&number:rem n m) 0
              recur (conj acc n) (inc n) limit
              recur acc (inc n) limit
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] (:: 'List 'Number) 'Number 'Number
            :return $ :: 'List 'Number
        'test-rem-methods! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn test-rem-methods! ()
            assert= 1 $ rem-direct 98 97
            assert= 47025 $ loop-rem-direct 1000 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'try-fibo $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn try-fibo ()
            let
                n 22
              println "|fibo result:" n $ fibo n
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'try-prime $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn try-prime ()
            println $ sieve-primes ([] 2 3 5 7 11 13) 17 400
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main (:require)

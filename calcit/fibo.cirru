
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |app) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/reload!)
      :modules $ []
      :type-slots $ {}
    :prime $ {} (:description |) (:init-fn 'app.main/try-prime) (:mode :native) (:reload-fn 'app.main/try-prime)
      :modules $ []
      :type-slots $ {}
  :files $ {}
    |app.main $ %{} 'FileEntry
      :defs $ {}
        |fibo $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn fibo (x)
              if (< x 2) 1 $ +
                fibo $ - x 1
                fibo $ - x 2
          :examples $ []
          :schema $ :: 'Dynamic
        |main! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn main! () (println "|Loaded program!") (try-fibo)
          :examples $ []
          :schema $ :: 'Dynamic
        |reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn reload! () $ :: 'Unit
          :examples $ []
          :schema $ :: 'Dynamic
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

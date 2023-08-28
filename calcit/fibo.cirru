
{} (:package |app)
  :configs $ {} (:init-fn |app.main/main!) (:reload-fn |app.main/reload!)
  :entries $ {}
    :prime $ {} (:init-fn |app.main/try-prime) (:reload-fn |app.main/try-prime)
      :modules $ []
  :files $ {}
    |app.main $ %{} :FileEntry
      :defs $ {}
        |fibo $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn fibo (x)
              if (< x 2) 1 $ +
                fibo $ - x 1
                fibo $ - x 2
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! () (println "\"Loaded program!") (try-fibo)
        |reload! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn reload! () $ :: :unit
        |sieve-primes $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn sieve-primes (acc n limit)
              if (&> n limit) acc $ if
                every? acc $ fn (m)
                  &> (.rem n m) 0
                recur (conj acc n) (inc n) limit
                recur acc (inc n) limit
        |try-fibo $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn try-fibo () $ let
                n 22
              println "\"fibo result:" n $ fibo n
        |try-prime $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn try-prime () $ println
              sieve-primes ([] 2 3 5 7 11 13) 17 400
      :ns $ %{} :CodeEntry (:doc |)
        :code $ quote
          ns app.main $ :require

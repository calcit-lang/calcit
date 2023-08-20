
{} (:package |app)
  :configs $ {} (:init-fn |app.main/main!) (:reload-fn |app.main/reload!)
  :entries $ {}
    :prime $ {} (:init-fn |app.main/try-prime) (:reload-fn |app.main/try-prime)
      :modules $ []
  :files $ {}
    |app.main $ {}
      :configs $ {}
      :defs $ {}
        |fibo $ %{} :CodeEntry
          :code $ quote
            defn fibo (x)
              if (< x 2) 1 $ +
                fibo $ - x 1
                fibo $ - x 2
          :doc |
        |main! $ %{} :CodeEntry
          :code $ quote
            defn main! () (println "\"Loaded program!") (try-fibo)
          :doc |
        |reload! $ %{} :CodeEntry
          :code $ quote
            defn reload! () $ :: :unit
          :doc |
        |sieve-primes $ %{} :CodeEntry
          :code $ quote
            defn sieve-primes (acc n limit)
              if (&> n limit) acc $ if
                every? acc $ fn (m)
                  &> (.rem n m) 0
                recur (conj acc n) (inc n) limit
                recur acc (inc n) limit
          :doc |
        |try-fibo $ %{} :CodeEntry
          :code $ quote
            defn try-fibo () $ let
                n 22
              println "\"fibo result:" n $ fibo n
          :doc |
        |try-prime $ %{} :CodeEntry
          :code $ quote
            defn try-prime () $ println
              sieve-primes ([] 2 3 5 7 11 13) 17 400
          :doc |
      :ns $ %{} :CodeEntry
        :code $ quote
          ns app.main $ :require
        :doc |
      :proc $ quote ()

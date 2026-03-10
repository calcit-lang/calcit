
{} (:about "|file is generated - never edit directly; learn cr edit/tree workflows before changing") (:package |app)
  :configs $ {} (:init-fn |app.main/main!) (:reload-fn |app.main/reload!) (:version |0.0.0)
    :modules $ []
  :entries $ {}
    :prime $ {} (:init-fn |app.main/try-prime) (:reload-fn |app.main/try-prime) (:version |0.0.0)
      :modules $ []
  :files $ {}
    |app.main $ %{} :FileEntry
      :defs $ {}
        |fibo $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn fibo (x)
              if (< x 2) 1 $ +
                fibo $ - x 1
                fibo $ - x 2
          :examples $ []
        |main! $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn main! () (println "\"Loaded program!") (try-fibo)
          :examples $ []
        |reload! $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn reload! () $ :: :unit
          :examples $ []
        |sieve-primes $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn sieve-primes (acc n limit)
              if (&> n limit) acc $ if
                every? acc $ fn (m)
                  &> (.rem n m) 0
                recur (conj acc n) (inc n) limit
                recur acc (inc n) limit
          :examples $ []
        |try-fibo $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn try-fibo () $ let
                n 22
              println "\"fibo result:" n $ fibo n
          :examples $ []
        |try-prime $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn try-prime () $ println
              sieve-primes ([] 2 3 5 7 11 13) 17 400
          :examples $ []
      :ns $ %{} :CodeEntry (:doc |) (:schema nil)
        :code $ quote
          ns app.main $ :require
        :examples $ []

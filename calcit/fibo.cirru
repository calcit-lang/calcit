
{} (:package |app)
  :configs $ {} (:init-fn |app.main/main!) (:reload-fn |app.main/reload!)
  :files $ {}
    |app.main $ {}
      :ns $ quote
        ns app.main $ :require
      :defs $ {}
        |main! $ quote
          defn main! ()
            println "\"Loaded program!"
            try-fibo
            ; echo $ sieve-primes ([] 2 3 5 7 11 13) 17 400

        |reload! $ quote
          defn reload! () nil

        |try-fibo $ quote
          defn try-fibo ()
            let
                n 22
              echo "\"fibo result:" n $ fibo n

        |fibo $ quote
          defn fibo (x)
            if (< x 2) (, 1)
              + (fibo $ - x 1) (fibo $ - x 2)

        |sieve-primes $ quote
          defn sieve-primes (acc n limit)
            if (&> n limit) acc $ if
              every? acc
                fn (m)
                  &> (.rem n m) 0
              recur (conj acc n) (inc n) (, limit)
              recur acc (inc n) limit

      :proc $ quote ()
      :configs $ {} (:extension nil)

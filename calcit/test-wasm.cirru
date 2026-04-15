
{} (:about "|WASM codegen test — pure numeric functions compiled to WAT") (:package |test-wasm)
  :configs $ {} (:init-fn |test-wasm.main/main!) (:reload-fn |test-wasm.main/reload!) (:version |0.0.0)
    :modules $ []
  :entries $ {}
  :files $ {}
    |test-wasm.main $ %{} :FileEntry
      :defs $ {}
        |fibo $ %{} :CodeEntry (:doc "|Fibonacci — recursive") (:schema nil)
          :code $ quote
            defn fibo (n)
              if (&< n 2) 1
                &+ (fibo (&- n 1)) (fibo (&- n 2))
          :examples $ []
        |factorial $ %{} :CodeEntry (:doc "|Factorial — recursive") (:schema nil)
          :code $ quote
            defn factorial (n)
              if (&< n 2) 1
                &* n $ factorial (&- n 1)
          :examples $ []
        |add-two $ %{} :CodeEntry (:doc "|Simple addition") (:schema nil)
          :code $ quote
            defn add-two (a b) (&+ a b)
          :examples $ []
        |sum-range-step $ %{} :CodeEntry (:doc "|Sum step helper: sum-range-step(acc, i, n)") (:schema nil)
          :code $ quote
            defn sum-range-step (acc i n)
              if (&> i n) acc
                recur (&+ acc i) (&+ i 1) n
          :examples $ []
        |sum-range $ %{} :CodeEntry (:doc "|Sum 1..n via helper") (:schema nil)
          :code $ quote
            defn sum-range (n) (sum-range-step 0 1 n)
          :examples $ []
        |test-floor $ %{} :CodeEntry (:doc "|floor function") (:schema nil)
          :code $ quote
            defn test-floor (x) (floor x)
          :examples $ []
        |test-ceil $ %{} :CodeEntry (:doc "|ceil function") (:schema nil)
          :code $ quote
            defn test-ceil (x) (ceil x)
          :examples $ []
        |test-round $ %{} :CodeEntry (:doc "|round function") (:schema nil)
          :code $ quote
            defn test-round (x) (round x)
          :examples $ []
        |test-sqrt $ %{} :CodeEntry (:doc "|sqrt function") (:schema nil)
          :code $ quote
            defn test-sqrt (x) (sqrt x)
          :examples $ []
        |test-rem $ %{} :CodeEntry (:doc "|remainder") (:schema nil)
          :code $ quote
            defn test-rem (a b) (&number:rem a b)
          :examples $ []
        |test-compare $ %{} :CodeEntry (:doc "|comparison chain") (:schema nil)
          :code $ quote
            defn test-compare (a b)
              if (&< a b) -1
                if (&> a b) 1 0
          :examples $ []
        |test-not $ %{} :CodeEntry (:doc "|not operation") (:schema nil)
          :code $ quote
            defn test-not (x) (not x)
          :examples $ []
        |test-let-chain $ %{} :CodeEntry (:doc "|chained let bindings") (:schema nil)
          :code $ quote
            defn test-let-chain (x)
              &let
                a $ &* x x
                &let
                  b $ &+ a 1
                  &* b 2
          :examples $ []
        |collatz-steps $ %{} :CodeEntry (:doc "|Collatz conjecture step counter") (:schema nil)
          :code $ quote
            defn collatz-steps (n)
              if (&< n 2) 0
                if (&= (&number:rem n 2) 0)
                  &+ 1 $ collatz-steps (&/ n 2)
                  &+ 1 $ collatz-steps (&+ (&* 3 n) 1)
          :examples $ []
        |gcd $ %{} :CodeEntry (:doc "|Greatest common divisor") (:schema nil)
          :code $ quote
            defn gcd (a b)
              if (&= b 0) a
                recur b $ &number:rem a b
          :examples $ []
        |main! $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn main! ()
              println $ fibo 10
          :examples $ []
        |reload! $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn reload! () nil
          :examples $ []
      :ns $ %{} :CodeEntry (:doc |)
        :code $ quote
          ns test-wasm.main


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
        |test-tag-eq $ %{} :CodeEntry (:doc "|Tag equality — same tags") (:schema nil)
          :code $ quote
            defn test-tag-eq ()
              if (&= :ok :ok) 1 0
          :examples $ []
        |test-tag-neq $ %{} :CodeEntry (:doc "|Tag inequality — different tags") (:schema nil)
          :code $ quote
            defn test-tag-neq ()
              if (&= :ok :err) 1 0
          :examples $ []
        |Point $ %{} :CodeEntry (:doc "|Record definition for WASM test") (:schema nil)
          :code $ quote
            defrecord Point :x :y
          :examples $ []
        |test-record-sum $ %{} :CodeEntry (:doc "|Record create + field access") (:schema nil)
          :code $ quote
            defn test-record-sum (x y)
              &let
                p $ %{} Point (:x x) (:y y)
                &+ (&record:nth p 0 :x) (&record:nth p 1 :y)
          :examples $ []
        |test-tuple-sum $ %{} :CodeEntry (:doc "|Tuple create + nth access") (:schema nil)
          :code $ quote
            defn test-tuple-sum ()
              &let
                t $ :: :pair 10 20
                &+ (&tuple:nth t 0) (&tuple:nth t 1)
          :examples $ []
        |test-bit-and $ %{} :CodeEntry (:doc "|Bitwise AND") (:schema nil)
          :code $ quote
            defn test-bit-and (a b) (bit-and a b)
          :examples $ []
        |test-bit-or $ %{} :CodeEntry (:doc "|Bitwise OR") (:schema nil)
          :code $ quote
            defn test-bit-or (a b) (bit-or a b)
          :examples $ []
        |test-bit-xor $ %{} :CodeEntry (:doc "|Bitwise XOR") (:schema nil)
          :code $ quote
            defn test-bit-xor (a b) (bit-xor a b)
          :examples $ []
        |test-bit-not $ %{} :CodeEntry (:doc "|Bitwise NOT") (:schema nil)
          :code $ quote
            defn test-bit-not (a) (bit-not a)
          :examples $ []
        |test-bit-shl $ %{} :CodeEntry (:doc "|Bitwise shift left") (:schema nil)
          :code $ quote
            defn test-bit-shl (a b) (bit-shl a b)
          :examples $ []
        |test-bit-shr $ %{} :CodeEntry (:doc "|Bitwise shift right") (:schema nil)
          :code $ quote
            defn test-bit-shr (a b) (bit-shr a b)
          :examples $ []
        |test-match-tag $ %{} :CodeEntry (:doc "|Match on tuple tag") (:schema nil)
          :code $ quote
            defn test-match-tag (x y)
              &let
                t $ :: :add x y
                match t
                  (:add a b) (&+ a b)
                  (:sub a b) (&- a b)
                  _ 0
          :examples $ []
        |test-match-sub $ %{} :CodeEntry (:doc "|Match on second variant") (:schema nil)
          :code $ quote
            defn test-match-sub (x y)
              &let
                t $ :: :sub x y
                match t
                  (:add a b) (&+ a b)
                  (:sub a b) (&- a b)
                  _ 0
          :examples $ []
        |test-match-wildcard $ %{} :CodeEntry (:doc "|Match falls to wildcard") (:schema nil)
          :code $ quote
            defn test-match-wildcard ()
              &let
                t $ :: :unknown 99
                match t
                  (:add a b) (&+ a b)
                  _ -1
          :examples $ []
        |test-pow $ %{} :CodeEntry (:doc "|pow via host import") (:schema nil)
          :code $ quote
            defn test-pow (base exp) (pow base exp)
          :examples $ []
        |test-sin $ %{} :CodeEntry (:doc "|sin via host import") (:schema nil)
          :code $ quote
            defn test-sin (x) (sin x)
          :examples $ []
        |test-cos $ %{} :CodeEntry (:doc "|cos via host import") (:schema nil)
          :code $ quote
            defn test-cos (x) (cos x)
          :examples $ []
        |test-cross-ns $ %{} :CodeEntry (:doc "|Cross-namespace function call") (:schema nil)
          :code $ quote
            defn test-cross-ns (a b)
              helper/add-and-double a b
          :examples $ []
        |test-abs $ %{} :CodeEntry (:doc "|abs from calcit.core") (:schema nil)
          :code $ quote
            defn test-abs (x) (abs x)
          :examples $ []
        |test-negate $ %{} :CodeEntry (:doc "|negate from calcit.core") (:schema nil)
          :code $ quote
            defn test-negate (x) (negate x)
          :examples $ []
        |test-lte $ %{} :CodeEntry (:doc "|less-than-or-equal") (:schema nil)
          :code $ quote
            defn test-lte (a b)
              if (&< a b) 1
                if (&= a b) 1 0
          :examples $ []
        |test-gte $ %{} :CodeEntry (:doc "|greater-than-or-equal") (:schema nil)
          :code $ quote
            defn test-gte (a b)
              if (&> a b) 1
                if (&= a b) 1 0
          :examples $ []
        |test-min $ %{} :CodeEntry (:doc "|min of two numbers") (:schema nil)
          :code $ quote
            defn test-min (a b)
              if (&< a b) a b
          :examples $ []
        |test-max $ %{} :CodeEntry (:doc "|max of two numbers") (:schema nil)
          :code $ quote
            defn test-max (a b)
              if (&> a b) a b
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
            :require
              test-wasm.helper :as helper
        :examples $ []
    |test-wasm.helper $ %{} :FileEntry
      :defs $ {}
        |add-and-double $ %{} :CodeEntry (:doc "|Helper: add two numbers and double") (:schema nil)
          :code $ quote
            defn add-and-double (a b)
              &* (&+ a b) 2
          :examples $ []
      :ns $ %{} :CodeEntry (:doc |)
        :code $ quote
          ns test-wasm.helper


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
        |test-list-count $ %{} :CodeEntry (:doc "|list count") (:schema nil)
          :code $ quote
            defn test-list-count ()
              &list:count $ [] 10 20 30
          :examples $ []
        |test-list-nth $ %{} :CodeEntry (:doc "|list nth element") (:schema nil)
          :code $ quote
            defn test-list-nth (i)
              &list:nth ([] 10 20 30 40) i
          :examples $ []
        |test-list-first $ %{} :CodeEntry (:doc "|list first element") (:schema nil)
          :code $ quote
            defn test-list-first ()
              &list:first $ [] 42 99
          :examples $ []
        |test-list-rest-count $ %{} :CodeEntry (:doc "|count of rest") (:schema nil)
          :code $ quote
            defn test-list-rest-count ()
              &list:count $ &list:rest $ [] 10 20 30
          :examples $ []
        |test-list-rest-first $ %{} :CodeEntry (:doc "|first of rest") (:schema nil)
          :code $ quote
            defn test-list-rest-first ()
              &list:first $ &list:rest $ [] 10 20 30
          :examples $ []
        |test-list-empty-true $ %{} :CodeEntry (:doc "|empty list is empty") (:schema nil)
          :code $ quote
            defn test-list-empty-true ()
              if (&list:empty? $ []) 1 0
          :examples $ []
        |test-list-empty-false $ %{} :CodeEntry (:doc "|non-empty list not empty") (:schema nil)
          :code $ quote
            defn test-list-empty-false ()
              if (&list:empty? $ [] 1) 1 0
          :examples $ []
        |test-list-append $ %{} :CodeEntry (:doc "|append returns correct count and last elem") (:schema nil)
          :code $ quote
            defn test-list-append ()
              &let
                xs $ append ([] 10 20) 30
                &+ (&list:count xs) (&list:nth xs 2)
          :examples $ []
        |test-list-prepend $ %{} :CodeEntry (:doc "|prepend returns correct first elem") (:schema nil)
          :code $ quote
            defn test-list-prepend ()
              &list:first $ prepend ([] 10 20) 5
          :examples $ []
        |test-list-butlast $ %{} :CodeEntry (:doc "|butlast drops last element") (:schema nil)
          :code $ quote
            defn test-list-butlast ()
              &list:count $ butlast $ [] 10 20 30
          :examples $ []
        |test-list-slice $ %{} :CodeEntry (:doc "|slice with start and end") (:schema nil)
          :code $ quote
            defn test-list-slice ()
              &let
                xs $ &list:slice ([] 10 20 30 40 50) 1 4
                &+ (&list:count xs) (&list:first xs)
          :examples $ []
        |test-list-reverse $ %{} :CodeEntry (:doc "|reverse a list") (:schema nil)
          :code $ quote
            defn test-list-reverse ()
              &let
                xs $ &list:reverse $ [] 10 20 30
                &+ (&list:first xs) (&list:nth xs 2)
          :examples $ []
        |test-list-concat $ %{} :CodeEntry (:doc "|concat two lists") (:schema nil)
          :code $ quote
            defn test-list-concat ()
              &let
                xs $ &list:concat ([] 10 20) ([] 30 40)
                &+ (&list:count xs) (&list:nth xs 3)
          :examples $ []
        |test-list-assoc $ %{} :CodeEntry (:doc "|assoc replaces element") (:schema nil)
          :code $ quote
            defn test-list-assoc ()
              &list:nth (&list:assoc ([] 10 20 30) 1 99) 1
          :examples $ []
        |test-list-dissoc $ %{} :CodeEntry (:doc "|dissoc removes element") (:schema nil)
          :code $ quote
            defn test-list-dissoc ()
              &let
                xs $ &list:dissoc ([] 10 20 30) 1
                &+ (&list:count xs) (&list:nth xs 1)
          :examples $ []
        |test-list-contains $ %{} :CodeEntry (:doc "|contains checks index bounds") (:schema nil)
          :code $ quote
            defn test-list-contains ()
              &let
                xs $ [] 10 20 30
                &+ (if (&list:contains? xs 2) 1 0) (if (&list:contains? xs 5) 10 0)
          :examples $ []
        |test-list-includes $ %{} :CodeEntry (:doc "|includes checks value presence") (:schema nil)
          :code $ quote
            defn test-list-includes ()
              &+ (if (&list:includes? ([] 10 20 30) 20) 1 0) (if (&list:includes? ([] 10 20 30) 99) 10 0)
          :examples $ []
        |test-map-count $ %{} :CodeEntry (:doc "|map count") (:schema nil)
          :code $ quote
            defn test-map-count ()
              &map:count $ &{} :a 1 :b 2 :c 3
          :examples $ []
        |test-map-get $ %{} :CodeEntry (:doc "|map get by key") (:schema nil)
          :code $ quote
            defn test-map-get ()
              &map:get (&{} :a 10 :b 20 :c 30) :b
          :examples $ []
        |test-map-empty-true $ %{} :CodeEntry (:doc "|empty map is empty") (:schema nil)
          :code $ quote
            defn test-map-empty-true ()
              if (&map:empty? $ &{}) 1 0
          :examples $ []
        |test-map-empty-false $ %{} :CodeEntry (:doc "|non-empty map not empty") (:schema nil)
          :code $ quote
            defn test-map-empty-false ()
              if (&map:empty? $ &{} :a 1) 1 0
          :examples $ []
        |test-map-assoc-new $ %{} :CodeEntry (:doc "|assoc adds new key") (:schema nil)
          :code $ quote
            defn test-map-assoc-new ()
              &let
                m $ &map:assoc (&{} :a 1) :b 2
                &+ (&map:count m) (&map:get m :b)
          :examples $ []
        |test-map-assoc-update $ %{} :CodeEntry (:doc "|assoc updates existing key") (:schema nil)
          :code $ quote
            defn test-map-assoc-update ()
              &map:get (&map:assoc (&{} :a 1 :b 2) :b 99) :b
          :examples $ []
        |test-map-dissoc $ %{} :CodeEntry (:doc "|dissoc removes key") (:schema nil)
          :code $ quote
            defn test-map-dissoc ()
              &let
                m $ &map:dissoc (&{} :a 1 :b 2 :c 3) :b
                &+ (&map:count m) (&map:get m :c)
          :examples $ []
        |test-map-contains $ %{} :CodeEntry (:doc "|contains checks key presence") (:schema nil)
          :code $ quote
            defn test-map-contains ()
              &+ (if (&map:contains? (&{} :a 1 :b 2) :a) 1 0) (if (&map:contains? (&{} :a 1 :b 2) :z) 10 0)
          :examples $ []
        |test-set-count $ %{} :CodeEntry (:doc "|set count") (:schema nil)
          :code $ quote
            defn test-set-count ()
              &set:count $ #{} 10 20 30
          :examples $ []
        |test-set-empty $ %{} :CodeEntry (:doc "|empty set") (:schema nil)
          :code $ quote
            defn test-set-empty ()
              &+ (if (&set:empty? $ #{}) 1 0) (if (&set:empty? $ #{} 1) 10 0)
          :examples $ []
        |test-set-includes $ %{} :CodeEntry (:doc "|set includes value") (:schema nil)
          :code $ quote
            defn test-set-includes ()
              &+ (if (&set:includes? (#{} 10 20 30) 20) 1 0) (if (&set:includes? (#{} 10 20 30) 99) 10 0)
          :examples $ []
        |test-set-include $ %{} :CodeEntry (:doc "|include adds element") (:schema nil)
          :code $ quote
            defn test-set-include ()
              &set:count $ &include (#{} 10 20) 30
          :examples $ []
        |test-set-exclude $ %{} :CodeEntry (:doc "|exclude removes element") (:schema nil)
          :code $ quote
            defn test-set-exclude ()
              &set:count $ &exclude (#{} 10 20 30) 20
          :examples $ []
        |test-to-pairs $ %{} :CodeEntry (:doc "|to-pairs count") (:schema nil)
          :code $ quote
            defn test-to-pairs ()
              &let
                ps $ to-pairs $ &{} :a 1 :b 2
                &+ (&list:count ps) (&list:count $ &list:first ps)
          :examples $ []
        |test-map-includes $ %{} :CodeEntry (:doc "|map includes checks value") (:schema nil)
          :code $ quote
            defn test-map-includes ()
              &+ (if (&map:includes? (&{} :a 10 :b 20) 20) 1 0) (if (&map:includes? (&{} :a 10 :b 20) 99) 10 0)
          :examples $ []
        |collect-rest $ %{} :CodeEntry (:doc "|returns rest list unchanged") (:schema nil)
          :code $ quote
            defn collect-rest (a & xs) xs
          :examples $ []
        |test-rest-count $ %{} :CodeEntry (:doc "|rest args count: 3 extras") (:schema nil)
          :code $ quote
            defn test-rest-count ()
              &list:count $ collect-rest 1 2 3 4
          :examples $ []
        |sum-rest-list $ %{} :CodeEntry (:doc "|helper: sums a list via recur") (:schema nil)
          :code $ quote
            defn sum-rest-list (acc xs)
              if (&list:empty? xs) acc
                recur (&+ acc (&list:first xs)) (&list:rest xs)
          :examples $ []
        |sum-rest $ %{} :CodeEntry (:doc "|variadic sum: a + b + rest...") (:schema nil)
          :code $ quote
            defn sum-rest (a b & xs)
              sum-rest-list (&+ a b) xs
          :examples $ []
        |test-rest-sum $ %{} :CodeEntry (:doc "|rest args: 1+2+3+4+5 = 15") (:schema nil)
          :code $ quote
            defn test-rest-sum ()
              sum-rest 1 2 3 4 5
          :examples $ []
        |test-rest-empty $ %{} :CodeEntry (:doc "|rest args with no extras: 10+20 = 30") (:schema nil)
          :code $ quote
            defn test-rest-empty ()
              sum-rest 10 20
          :examples $ []
        |test-type-of-list $ %{} :CodeEntry (:doc "|type-of list == :list tag") (:schema nil)
          :code $ quote
            defn test-type-of-list ()
              if (&= (type-of ([] 1 2 3)) :list) 1 0
          :examples $ []
        |test-type-of-map $ %{} :CodeEntry (:doc "|type-of map == :map tag") (:schema nil)
          :code $ quote
            defn test-type-of-map ()
              if (&= (type-of (&{} :a 1)) :map) 1 0
          :examples $ []
        |test-type-of-set $ %{} :CodeEntry (:doc "|type-of set == :set tag") (:schema nil)
          :code $ quote
            defn test-type-of-set ()
              if (&= (type-of (#{} 1 2)) :set) 1 0
          :examples $ []
        |test-type-of-number $ %{} :CodeEntry (:doc "|type-of number == :number tag") (:schema nil)
          :code $ quote
            defn test-type-of-number ()
              if (&= (type-of 42) :number) 1 0
          :examples $ []
        |test-type-of-tuple $ %{} :CodeEntry (:doc "|type-of tuple == :tuple tag") (:schema nil)
          :code $ quote
            defn test-type-of-tuple ()
              if (&= (type-of (:: :Pair 1 2)) :tuple) 1 0
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

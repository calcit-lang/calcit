
{} (:about "|Machine-generated snapshot. AI AGENTS: never edit this file directly — changes will be overwritten on recompile. Inspect via `cr query`; modify via `cr edit` / `cr tree`. MANDATORY first step: run `cr docs agents --full`.") (:package |test-wasm)
  :configs $ {} (:init-fn |test-wasm.main/main!) (:reload-fn |test-wasm.main/reload!) (:version |0.0.0)
    :modules $ []
  :entries $ {}
  :files $ {}
    |test-wasm.helper $ %{} :FileEntry
      :defs $ {}
        |add-and-double $ %{} :CodeEntry (:doc "|Helper: add two numbers and double") (:schema :dynamic)
          :code $ quote
            defn add-and-double (a b)
              &* (&+ a b) 2
          :examples $ []
      :ns $ %{} :NsEntry (:doc |)
        :code $ quote (ns test-wasm.helper)
    |test-wasm.main $ %{} :FileEntry
      :defs $ {}
        |Point $ %{} :CodeEntry (:doc "|Record definition for WASM test") (:schema :dynamic)
          :code $ quote (defrecord Point :x :y)
          :examples $ []
        |add-two $ %{} :CodeEntry (:doc "|Simple addition") (:schema :dynamic)
          :code $ quote
            defn add-two (a b) (&+ a b)
          :examples $ []
        |collatz-steps $ %{} :CodeEntry (:doc "|Collatz conjecture step counter") (:schema :dynamic)
          :code $ quote
            defn collatz-steps (n)
              if (&< n 2) 0 $ if
                &= (&number:rem n 2) 0
                &+ 1 $ collatz-steps (&/ n 2)
                &+ 1 $ collatz-steps
                  &+ (&* 3 n) 1
          :examples $ []
        |collect-rest $ %{} :CodeEntry (:doc "|returns rest list unchanged") (:schema :dynamic)
          :code $ quote
            defn collect-rest (a & xs) xs
          :examples $ []
        |factorial $ %{} :CodeEntry (:doc "|Factorial — recursive") (:schema :dynamic)
          :code $ quote
            defn factorial (n)
              if (&< n 2) 1 $ &* n
                factorial $ &- n 1
          :examples $ []
        |fibo $ %{} :CodeEntry (:doc "|Fibonacci — recursive") (:schema :dynamic)
          :code $ quote
            defn fibo (n)
              if (&< n 2) 1 $ &+
                fibo $ &- n 1
                fibo $ &- n 2
          :examples $ []
        |gcd $ %{} :CodeEntry (:doc "|Greatest common divisor") (:schema :dynamic)
          :code $ quote
            defn gcd (a b)
              if (&= b 0) a $ recur b (&number:rem a b)
          :examples $ []
        |main! $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defn main! () $ println (fibo 10)
          :examples $ []
        |reload! $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defn reload! () nil
          :examples $ []
        |sum-range $ %{} :CodeEntry (:doc "|Sum 1..n via helper") (:schema :dynamic)
          :code $ quote
            defn sum-range (n) (sum-range-step 0 1 n)
          :examples $ []
        |sum-range-step $ %{} :CodeEntry (:doc "|Sum step helper: sum-range-step(acc, i, n)") (:schema :dynamic)
          :code $ quote
            defn sum-range-step (acc i n)
              if (&> i n) acc $ recur (&+ acc i) (&+ i 1) n
          :examples $ []
        |sum-rest $ %{} :CodeEntry (:doc "|variadic sum: a + b + rest...") (:schema :dynamic)
          :code $ quote
            defn sum-rest (a b & xs)
              sum-rest-list (&+ a b) xs
          :examples $ []
        |sum-rest-list $ %{} :CodeEntry (:doc "|helper: sums a list via recur") (:schema :dynamic)
          :code $ quote
            defn sum-rest-list (acc xs)
              if (&list:empty? xs) acc $ recur
                &+ acc $ &list:first xs
                &list:rest xs
          :examples $ []
        |test-abs $ %{} :CodeEntry (:doc "|abs from calcit.core") (:schema :dynamic)
          :code $ quote
            defn test-abs (x) (abs x)
          :examples $ []
        |test-bit-and $ %{} :CodeEntry (:doc "|Bitwise AND") (:schema :dynamic)
          :code $ quote
            defn test-bit-and (a b) (bit-and a b)
          :examples $ []
        |test-bit-not $ %{} :CodeEntry (:doc "|Bitwise NOT") (:schema :dynamic)
          :code $ quote
            defn test-bit-not (a) (bit-not a)
          :examples $ []
        |test-bit-or $ %{} :CodeEntry (:doc "|Bitwise OR") (:schema :dynamic)
          :code $ quote
            defn test-bit-or (a b) (bit-or a b)
          :examples $ []
        |test-bit-shl $ %{} :CodeEntry (:doc "|Bitwise shift left") (:schema :dynamic)
          :code $ quote
            defn test-bit-shl (a b) (bit-shl a b)
          :examples $ []
        |test-bit-shr $ %{} :CodeEntry (:doc "|Bitwise shift right") (:schema :dynamic)
          :code $ quote
            defn test-bit-shr (a b) (bit-shr a b)
          :examples $ []
        |test-bit-xor $ %{} :CodeEntry (:doc "|Bitwise XOR") (:schema :dynamic)
          :code $ quote
            defn test-bit-xor (a b) (bit-xor a b)
          :examples $ []
        |test-buf-list-push $ %{} :CodeEntry (:doc "||buf-list push 3 items, count=3") (:schema :dynamic)
          :code $ quote
            defn test-buf-list-push () $ let
                buf $ &buf-list:new
              &buf-list:push buf 10
              &buf-list:push buf 20
              &buf-list:push buf 30
              &buf-list:count buf
          :examples $ []
        |test-buf-list-to-list $ %{} :CodeEntry (:doc "||buf-list concat [1,2,3] then to-list, count=3") (:schema :dynamic)
          :code $ quote
            defn test-buf-list-to-list () $ let
                buf $ &buf-list:new
                items $ [] 1 2 3
              &buf-list:concat buf items
              &list:count $ &buf-list:to-list buf
          :examples $ []
        |test-ceil $ %{} :CodeEntry (:doc "|ceil function") (:schema :dynamic)
          :code $ quote
            defn test-ceil (x) (ceil x)
          :examples $ []
        |test-compare $ %{} :CodeEntry (:doc "|comparison chain") (:schema :dynamic)
          :code $ quote
            defn test-compare (a b)
              if (&< a b) -1 $ if (&> a b) 1 0
          :examples $ []
        |test-cos $ %{} :CodeEntry (:doc "|cos via host import") (:schema :dynamic)
          :code $ quote
            defn test-cos (x) (cos x)
          :examples $ []
        |test-cross-ns $ %{} :CodeEntry (:doc "|Cross-namespace function call") (:schema :dynamic)
          :code $ quote
            defn test-cross-ns (a b) (helper/add-and-double a b)
          :examples $ []
        |test-floor $ %{} :CodeEntry (:doc "|floor function") (:schema :dynamic)
          :code $ quote
            defn test-floor (x) (floor x)
          :examples $ []
        |test-gte $ %{} :CodeEntry (:doc |greater-than-or-equal) (:schema :dynamic)
          :code $ quote
            defn test-gte (a b)
              if (&> a b) 1 $ if (&= a b) 1 0
          :examples $ []
        |test-let-chain $ %{} :CodeEntry (:doc "|chained let bindings") (:schema :dynamic)
          :code $ quote
            defn test-let-chain (x)
              &let
                a $ &* x x
                &let
                  b $ &+ a 1
                  &* b 2
          :examples $ []
        |test-list-append $ %{} :CodeEntry (:doc "|append returns correct count and last elem") (:schema :dynamic)
          :code $ quote
            defn test-list-append () $ &let
              xs $ append ([] 10 20) 30
              &+ (&list:count xs) (&list:nth xs 2)
          :examples $ []
        |test-list-assoc $ %{} :CodeEntry (:doc "|assoc replaces element") (:schema :dynamic)
          :code $ quote
            defn test-list-assoc () $ &list:nth
              &list:assoc ([] 10 20 30) 1 99
              , 1
          :examples $ []
        |test-list-butlast $ %{} :CodeEntry (:doc "|butlast drops last element") (:schema :dynamic)
          :code $ quote
            defn test-list-butlast () $ &list:count
              butlast $ [] 10 20 30
          :examples $ []
        |test-list-concat $ %{} :CodeEntry (:doc "|concat two lists") (:schema :dynamic)
          :code $ quote
            defn test-list-concat () $ &let
              xs $ &list:concat ([] 10 20) ([] 30 40)
              &+ (&list:count xs) (&list:nth xs 3)
          :examples $ []
        |test-list-contains $ %{} :CodeEntry (:doc "|contains checks index bounds") (:schema :dynamic)
          :code $ quote
            defn test-list-contains () $ &let
              xs $ [] 10 20 30
              &+
                if (&list:contains? xs 2) 1 0
                if (&list:contains? xs 5) 10 0
          :examples $ []
        |test-list-count $ %{} :CodeEntry (:doc "|list count") (:schema :dynamic)
          :code $ quote
            defn test-list-count () $ &list:count ([] 10 20 30)
          :examples $ []
        |test-list-dissoc $ %{} :CodeEntry (:doc "|dissoc removes element") (:schema :dynamic)
          :code $ quote
            defn test-list-dissoc () $ &let
              xs $ &list:dissoc ([] 10 20 30) 1
              &+ (&list:count xs) (&list:nth xs 1)
          :examples $ []
        |test-list-empty-false $ %{} :CodeEntry (:doc "|non-empty list not empty") (:schema :dynamic)
          :code $ quote
            defn test-list-empty-false () $ if
              &list:empty? $ [] 1
              , 1 0
          :examples $ []
        |test-list-empty-true $ %{} :CodeEntry (:doc "|empty list is empty") (:schema :dynamic)
          :code $ quote
            defn test-list-empty-true () $ if
              &list:empty? $ []
              , 1 0
          :examples $ []
        |test-list-first $ %{} :CodeEntry (:doc "|list first element") (:schema :dynamic)
          :code $ quote
            defn test-list-first () $ &list:first ([] 42 99)
          :examples $ []
        |test-list-first-generic $ %{} :CodeEntry (:doc "|generic first() on list via invoke") (:schema :dynamic)
          :code $ quote
            defn test-list-first-generic () $ first ([] 42 99)
          :examples $ []
        |test-list-includes $ %{} :CodeEntry (:doc "|includes checks value presence") (:schema :dynamic)
          :code $ quote
            defn test-list-includes () $ &+
              if
                &list:includes? ([] 10 20 30) 20
                , 1 0
              if
                &list:includes? ([] 10 20 30) 99
                , 10 0
          :examples $ []
        |test-list-nth $ %{} :CodeEntry (:doc "|list nth element") (:schema :dynamic)
          :code $ quote
            defn test-list-nth (i)
              &list:nth ([] 10 20 30 40) i
          :examples $ []
        |test-list-prepend $ %{} :CodeEntry (:doc "|prepend returns correct first elem") (:schema :dynamic)
          :code $ quote
            defn test-list-prepend () $ &list:first
              prepend ([] 10 20) 5
          :examples $ []
        |test-list-rest-count $ %{} :CodeEntry (:doc "|count of rest") (:schema :dynamic)
          :code $ quote
            defn test-list-rest-count () $ &list:count
              &list:rest $ [] 10 20 30
          :examples $ []
        |test-list-rest-first $ %{} :CodeEntry (:doc "|first of rest") (:schema :dynamic)
          :code $ quote
            defn test-list-rest-first () $ &list:first
              &list:rest $ [] 10 20 30
          :examples $ []
        |test-list-rest-generic-first $ %{} :CodeEntry (:doc "|generic rest() on list via invoke") (:schema :dynamic)
          :code $ quote
            defn test-list-rest-generic-first () $ first
              rest $ [] 10 20 30
          :examples $ []
        |test-list-reverse $ %{} :CodeEntry (:doc "|reverse a list") (:schema :dynamic)
          :code $ quote
            defn test-list-reverse () $ &let
              xs $ &list:reverse ([] 10 20 30)
              &+ (&list:first xs) (&list:nth xs 2)
          :examples $ []
        |test-list-slice $ %{} :CodeEntry (:doc "|slice with start and end") (:schema :dynamic)
          :code $ quote
            defn test-list-slice () $ &let
              xs $ &list:slice ([] 10 20 30 40 50) 1 4
              &+ (&list:count xs) (&list:first xs)
          :examples $ []
        |test-list?-false $ %{} :CodeEntry (:doc "|list? on number returns false (0)") (:schema :dynamic)
          :code $ quote
            defn test-list?-false () $ if (list? 42) 1 0
          :examples $ []
        |test-list?-true $ %{} :CodeEntry (:doc "|list? on a list returns true (1)") (:schema :dynamic)
          :code $ quote
            defn test-list?-true () $ if
              list? $ [] 1 2
              , 1 0
          :examples $ []
        |test-lte $ %{} :CodeEntry (:doc |less-than-or-equal) (:schema :dynamic)
          :code $ quote
            defn test-lte (a b)
              if (&< a b) 1 $ if (&= a b) 1 0
          :examples $ []
        |test-map-assoc-new $ %{} :CodeEntry (:doc "|assoc adds new key") (:schema :dynamic)
          :code $ quote
            defn test-map-assoc-new () $ &let
              m $ &map:assoc (&{} :a 1) :b 2
              &+ (&map:count m) (&map:get m :b)
          :examples $ []
        |test-map-assoc-update $ %{} :CodeEntry (:doc "|assoc updates existing key") (:schema :dynamic)
          :code $ quote
            defn test-map-assoc-update () $ &map:get
              &map:assoc (&{} :a 1 :b 2) :b 99
              , :b
          :examples $ []
        |test-map-bucket-update $ %{} :CodeEntry (:doc "|update on collided numeric keys keeps lookup correct") (:schema :dynamic)
          :code $ quote
            defn test-map-bucket-update (a b) $ &let
              m $ &map:assoc (&{} a 10 b 20) b 99
              &+ (&map:get m a) (&map:get m b)
          :examples $ []
        |test-map-common-keys $ %{} :CodeEntry (:doc "|common-keys: keys in both a and b") (:schema :dynamic)
          :code $ quote
            defn test-map-common-keys () $ &set:count
              &map:common-keys (&{} :a 1 :b 2 :c 3) (&{} :b 10 :c 20 :d 30)
          :examples $ []
        |test-map-contains $ %{} :CodeEntry (:doc "|contains checks key presence") (:schema :dynamic)
          :code $ quote
            defn test-map-contains () $ &+
              if
                &map:contains? (&{} :a 1 :b 2) :a
                , 1 0
              if
                &map:contains? (&{} :a 1 :b 2) :z
                , 10 0
          :examples $ []
        |test-map-count $ %{} :CodeEntry (:doc "|map count") (:schema :dynamic)
          :code $ quote
            defn test-map-count () $ &map:count (&{} :a 1 :b 2 :c 3)
          :examples $ []
        |test-map-diff-keys $ %{} :CodeEntry (:doc "|diff-keys: keys in a not in b") (:schema :dynamic)
          :code $ quote
            defn test-map-diff-keys () $ &set:count
              &map:diff-keys (&{} :a 1 :b 2 :c 3) (&{} :b 10)
          :examples $ []
        |test-map-diff-new $ %{} :CodeEntry (:doc "|diff-new: entries in b not in a") (:schema :dynamic)
          :code $ quote
            defn test-map-diff-new () $ &map:count
              &map:diff-new (&{} :a 1 :b 2) (&{} :b 3 :c 4 :d 5)
          :examples $ []
        |test-map-dissoc $ %{} :CodeEntry (:doc "|dissoc removes key") (:schema :dynamic)
          :code $ quote
            defn test-map-dissoc () $ &let
              m $ &map:dissoc (&{} :a 1 :b 2 :c 3) :b
              &+ (&map:count m) (&map:get m :c)
          :examples $ []
        |test-map-empty-false $ %{} :CodeEntry (:doc "|non-empty map not empty") (:schema :dynamic)
          :code $ quote
            defn test-map-empty-false () $ if
              &map:empty? $ &{} :a 1
              , 1 0
          :examples $ []
        |test-map-empty-true $ %{} :CodeEntry (:doc "|empty map is empty") (:schema :dynamic)
          :code $ quote
            defn test-map-empty-true () $ if
              &map:empty? $ &{}
              , 1 0
          :examples $ []
        |test-map-get $ %{} :CodeEntry (:doc "|map get by key") (:schema :dynamic)
          :code $ quote
            defn test-map-get () $ &map:get (&{} :a 10 :b 20 :c 30) :b
          :examples $ []
        |test-map-hash-index1 $ %{} :CodeEntry (:doc "|second 5 bits of number hash") (:schema :dynamic)
          :code $ quote
            defn test-map-hash-index1 (n) $ bit-and (bit-shr (&hash n) 5) 31
          :examples $ []
        |test-map-hash-value $ %{} :CodeEntry (:doc "|raw hash for numeric key") (:schema :dynamic)
          :code $ quote
            defn test-map-hash-value (n) $ &hash n
          :examples $ []
        |test-map-includes $ %{} :CodeEntry (:doc "|map includes checks value") (:schema :dynamic)
          :code $ quote
            defn test-map-includes () $ &+
              if
                &map:includes? (&{} :a 10 :b 20) 20
                , 1 0
              if
                &map:includes? (&{} :a 10 :b 20) 99
                , 10 0
          :examples $ []
        |test-map-merge $ %{} :CodeEntry (:doc "|merge two maps, b overrides a") (:schema :dynamic)
          :code $ quote
            defn test-map-merge () $ &map:count
              &merge (&{} :a 1 :b 2) (&{} :b 3 :c 4)
          :examples $ []
        |test-map-merge-value $ %{} :CodeEntry (:doc "|merge override check via get") (:schema :dynamic)
          :code $ quote
            defn test-map-merge-value () $ &map:get
              &merge (&{} :a 1 :b 2) (&{} :b 99)
              , :b
          :examples $ []
        |test-map-two-keys-sum $ %{} :CodeEntry (:doc "|sum lookups for two numeric keys") (:schema :dynamic)
          :code $ quote
            defn test-map-two-keys-sum (a b) $ &let
              m $ &{} a 10 b 20
              &+ (&map:get m a) (&map:get m b)
          :examples $ []
        |test-map?-true $ %{} :CodeEntry (:doc "|map? on map returns true (1)") (:schema :dynamic)
          :code $ quote
            defn test-map?-true () $ if
              map? $ &{} :a 1
              , 1 0
          :examples $ []
        |test-match-sub $ %{} :CodeEntry (:doc "|Match on second variant") (:schema :dynamic)
          :code $ quote
            defn test-match-sub (x y)
              &let
                t $ :: :sub x y
                match t
                    :add a b
                    &+ a b
                  (:sub a b) (&- a b)
                  _ 0
          :examples $ []
        |test-match-tag $ %{} :CodeEntry (:doc "|Match on tuple tag") (:schema :dynamic)
          :code $ quote
            defn test-match-tag (x y)
              &let
                t $ :: :add x y
                match t
                    :add a b
                    &+ a b
                  (:sub a b) (&- a b)
                  _ 0
          :examples $ []
        |test-match-wildcard $ %{} :CodeEntry (:doc "|Match falls to wildcard") (:schema :dynamic)
          :code $ quote
            defn test-match-wildcard () $ &let
              t $ :: :unknown 99
              match t
                  :add a b
                  &+ a b
                _ -1
          :examples $ []
        |test-hash-number $ %{} :CodeEntry (:doc "|hash on number returns stable non-zero value") (:schema :dynamic)
          :code $ quote
            defn test-hash-number () $ if
              &> (&hash 42) 0
              , 1 0
          :examples $ []
        |test-max $ %{} :CodeEntry (:doc "|max of two numbers") (:schema :dynamic)
          :code $ quote
            defn test-max (a b)
              if (&> a b) a b
          :examples $ []
        |test-min $ %{} :CodeEntry (:doc "|min of two numbers") (:schema :dynamic)
          :code $ quote
            defn test-min (a b)
              if (&< a b) a b
          :examples $ []
        |test-negate $ %{} :CodeEntry (:doc "|negate from calcit.core") (:schema :dynamic)
          :code $ quote
            defn test-negate (x) (negate x)
          :examples $ []
        |test-not $ %{} :CodeEntry (:doc "|not operation") (:schema :dynamic)
          :code $ quote
            defn test-not (x) (not x)
          :examples $ []
        |test-number?-true $ %{} :CodeEntry (:doc "|number? on number returns true (1)") (:schema :dynamic)
          :code $ quote
            defn test-number?-true () $ if (number? 42) 1 0
          :examples $ []
        |test-pow $ %{} :CodeEntry (:doc "|pow via host import") (:schema :dynamic)
          :code $ quote
            defn test-pow (base exp) (pow base exp)
          :examples $ []
        |test-println $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defn test-println () do (println 42) 1
          :examples $ []
        |test-range $ %{} :CodeEntry (:doc "|range creates list of numbers") (:schema :dynamic)
          :code $ quote
            defn test-range () $ &list:count (range 5)
          :examples $ []
        |test-range-sum $ %{} :CodeEntry (:doc "|range 5 first+last: 0+4=4") (:schema :dynamic)
          :code $ quote
            defn test-range-sum () $ &let
              xs $ range 5
              &+ (&list:nth xs 0) (&list:nth xs 4)
          :examples $ []
        |test-range-two-args $ %{} :CodeEntry (:doc "|range 2 5 creates 3 elements") (:schema :dynamic)
          :code $ quote
            defn test-range-two-args () $ &list:count (range 2 5)
          :examples $ []
        |test-record-matches-true $ %{} :CodeEntry (:doc "|record:matches? returns true for same type") (:schema :dynamic)
          :code $ quote
            defn test-record-matches-true () $ &let
              a $ %{} Point (:x 1) (:y 2)
              &let
                b $ %{} Point (:x 3) (:y 4)
                if (&record:matches? a b) 1 0
          :examples $ []
        |test-record-field-tag $ %{} :CodeEntry (:doc "|record field-tag resolves by index") (:schema :dynamic)
          :code $ quote
            defn test-record-field-tag () $ &let
              point $ %{} Point (:x 1) (:y 2)
              if
                &= (&record:field-tag point 0) :x
                , 1 0
          :examples $ []
        |test-record-sum $ %{} :CodeEntry (:doc "|Record create + field access") (:schema :dynamic)
          :code $ quote
            defn test-record-sum (x y)
              &let
                p $ %{} Point (:x x) (:y y)
                &+ (&record:nth p 0 :x) (&record:nth p 1 :y)
          :examples $ []
        |test-rem $ %{} :CodeEntry (:doc |remainder) (:schema :dynamic)
          :code $ quote
            defn test-rem (a b) (&number:rem a b)
          :examples $ []
        |test-rest-count $ %{} :CodeEntry (:doc "|rest args count: 3 extras") (:schema :dynamic)
          :code $ quote
            defn test-rest-count () $ &list:count (collect-rest 1 2 3 4)
          :examples $ []
        |test-rest-empty $ %{} :CodeEntry (:doc "|rest args with no extras: 10+20 = 30") (:schema :dynamic)
          :code $ quote
            defn test-rest-empty () $ sum-rest 10 20
          :examples $ []
        |test-rest-sum $ %{} :CodeEntry (:doc "|rest args: 1+2+3+4+5 = 15") (:schema :dynamic)
          :code $ quote
            defn test-rest-sum () $ sum-rest 1 2 3 4 5
          :examples $ []
        |test-round $ %{} :CodeEntry (:doc "|round function") (:schema :dynamic)
          :code $ quote
            defn test-round (x) (round x)
          :examples $ []
        |test-set-count $ %{} :CodeEntry (:doc "|set count") (:schema :dynamic)
          :code $ quote
            defn test-set-count () $ &set:count (#{} 10 20 30)
          :examples $ []
        |test-set-difference $ %{} :CodeEntry (:doc "|difference removes elements in second set") (:schema :dynamic)
          :code $ quote
            defn test-set-difference () $ &set:count
              &difference (#{} 10 20 30 40) (#{} 20 40)
          :examples $ []
        |test-set-difference-empty $ %{} :CodeEntry (:doc "|difference with disjoint sets keeps all") (:schema :dynamic)
          :code $ quote
            defn test-set-difference-empty () $ &set:count
              &difference (#{} 10 20) (#{} 30 40)
          :examples $ []
        |test-set-empty $ %{} :CodeEntry (:doc "|empty set") (:schema :dynamic)
          :code $ quote
            defn test-set-empty () $ &+
              if
                &set:empty? $ #{}
                , 1 0
              if
                &set:empty? $ #{} 1
                , 10 0
          :examples $ []
        |test-set-exclude $ %{} :CodeEntry (:doc "|exclude removes element") (:schema :dynamic)
          :code $ quote
            defn test-set-exclude () $ &set:count
              &exclude (#{} 10 20 30) 20
          :examples $ []
        |test-set-include $ %{} :CodeEntry (:doc "|include adds element") (:schema :dynamic)
          :code $ quote
            defn test-set-include () $ &set:count
              &include (#{} 10 20) 30
          :examples $ []
        |test-set-includes $ %{} :CodeEntry (:doc "|set includes value") (:schema :dynamic)
          :code $ quote
            defn test-set-includes () $ &+
              if
                &set:includes? (#{} 10 20 30) 20
                , 1 0
              if
                &set:includes? (#{} 10 20 30) 99
                , 10 0
          :examples $ []
        |test-set-union $ %{} :CodeEntry (:doc "|union merges two sets") (:schema :dynamic)
          :code $ quote
            defn test-set-union () $ &set:count
              &union (#{} 10 20) (#{} 20 30 40)
          :examples $ []
        |test-set-union-same $ %{} :CodeEntry (:doc "|union of identical sets") (:schema :dynamic)
          :code $ quote
            defn test-set-union-same () $ &set:count
              &union (#{} 10 20 30) (#{} 10 20 30)
          :examples $ []
        |test-sin $ %{} :CodeEntry (:doc "|sin via host import") (:schema :dynamic)
          :code $ quote
            defn test-sin (x) (sin x)
          :examples $ []
        |test-sqrt $ %{} :CodeEntry (:doc "|sqrt function") (:schema :dynamic)
          :code $ quote
            defn test-sqrt (x) (sqrt x)
          :examples $ []
        |test-tag-eq $ %{} :CodeEntry (:doc "|Tag equality — same tags") (:schema :dynamic)
          :code $ quote
            defn test-tag-eq () $ if (&= :ok :ok) 1 0
          :examples $ []
        |test-tag-neq $ %{} :CodeEntry (:doc "|Tag inequality — different tags") (:schema :dynamic)
          :code $ quote
            defn test-tag-neq () $ if (&= :ok :err) 1 0
          :examples $ []
        |test-to-pairs $ %{} :CodeEntry (:doc "|to-pairs count") (:schema :dynamic)
          :code $ quote
            defn test-to-pairs () $ &let
              ps $ to-pairs (&{} :a 1 :b 2)
              &+ (&list:count ps)
                &list:count $ &list:first ps
          :examples $ []
        |test-tuple-count $ %{} :CodeEntry (:doc "|Tuple count returns payload count") (:schema :dynamic)
          :code $ quote
            defn test-tuple-count () $ &let
              t $ :: :pair 10 20
              &tuple:count t
          :examples $ []
        |test-tuple-sum $ %{} :CodeEntry (:doc "|Tuple create + nth access: idx 1 and 2 are payloads") (:schema :dynamic)
          :code $ quote
            defn test-tuple-sum () $ &let
              t $ :: :pair 10 20
              &+ (&tuple:nth t 1) (&tuple:nth t 2)
          :examples $ []
        |test-tuple-assoc $ %{} :CodeEntry (:doc "|Tuple assoc updates payload by index") (:schema :dynamic)
          :code $ quote
            defn test-tuple-assoc () $ &let
              t $ &tuple:assoc (:: :pair 10 20) 1 9
              &+ (&tuple:nth t 1) (&tuple:nth t 2)
          :examples $ []
        |test-type-of-list $ %{} :CodeEntry (:doc "|type-of list == :list tag") (:schema :dynamic)
          :code $ quote
            defn test-type-of-list () $ if
              &=
                type-of $ [] 1 2 3
                , :list
              , 1 0
          :examples $ []
        |test-type-of-map $ %{} :CodeEntry (:doc "|type-of map == :map tag") (:schema :dynamic)
          :code $ quote
            defn test-type-of-map () $ if
              &=
                type-of $ &{} :a 1
                , :map
              , 1 0
          :examples $ []
        |test-type-of-number $ %{} :CodeEntry (:doc "|type-of number == :number tag") (:schema :dynamic)
          :code $ quote
            defn test-type-of-number () $ if
              &= (type-of 42) :number
              , 1 0
          :examples $ []
        |test-type-of-set $ %{} :CodeEntry (:doc "|type-of set == :set tag") (:schema :dynamic)
          :code $ quote
            defn test-type-of-set () $ if
              &=
                type-of $ #{} 1 2
                , :set
              , 1 0
          :examples $ []
        |test-type-of-tuple $ %{} :CodeEntry (:doc "|type-of tuple == :tuple tag") (:schema :dynamic)
          :code $ quote
            defn test-type-of-tuple () $ if
              &=
                type-of $ :: :Pair 1 2
                , :tuple
              , 1 0
          :examples $ []
      :ns $ %{} :NsEntry (:doc |)
        :code $ quote
          ns test-wasm.main $ :require (test-wasm.helper :as helper)

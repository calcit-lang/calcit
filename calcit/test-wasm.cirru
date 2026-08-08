
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |test-wasm) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'test-wasm.main/main!) (:mode :native) (:reload-fn 'test-wasm.main/reload!)
      :modules $ []
      :type-slots $ {}
  :files $ {}
    |test-wasm.helper $ %{} 'FileEntry
      :defs $ {}
        |add-and-double $ %{} 'CodeEntry (:doc "|Helper: add two numbers and double")
          :code $ quote
            defn add-and-double (a b)
              &* (&+ a b) 2
          :examples $ []
          :schema $ :: 'Dynamic
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote (ns test-wasm.helper)
    |test-wasm.main $ %{} 'FileEntry
      :defs $ {}
        |Point $ %{} 'CodeEntry (:doc "|Record definition for WASM test")
          :code $ quote (defrecord Point :x :y)
          :examples $ []
          :schema $ :: 'Dynamic
        |add-two $ %{} 'CodeEntry (:doc "|Simple addition")
          :code $ quote
            defn add-two (a b) (&+ a b)
          :examples $ []
          :schema $ :: 'Dynamic
        |collatz-steps $ %{} 'CodeEntry (:doc "|Collatz conjecture step counter")
          :code $ quote
            defn collatz-steps (n)
              if (&< n 2) 0 $ if
                &= (&number:rem n 2) 0
                &+ 1 $ collatz-steps (&/ n 2)
                &+ 1 $ collatz-steps
                  &+ (&* 3 n) 1
          :examples $ []
          :schema $ :: 'Dynamic
        |collect-rest $ %{} 'CodeEntry (:doc "|returns rest list unchanged")
          :code $ quote
            defn collect-rest (a & xs) xs
          :examples $ []
          :schema $ :: 'Dynamic
        |factorial $ %{} 'CodeEntry (:doc "|Factorial — recursive")
          :code $ quote
            defn factorial (n)
              if (&< n 2) 1 $ &* n
                factorial $ &- n 1
          :examples $ []
          :schema $ :: 'Dynamic
        |fibo $ %{} 'CodeEntry (:doc "|Fibonacci — recursive")
          :code $ quote
            defn fibo (n)
              if (&< n 2) 1 $ &+
                fibo $ &- n 1
                fibo $ &- n 2
          :examples $ []
          :schema $ :: 'Dynamic
        |gcd $ %{} 'CodeEntry (:doc "|Greatest common divisor")
          :code $ quote
            defn gcd (a b)
              if (&= b 0) a $ recur b (&number:rem a b)
          :examples $ []
          :schema $ :: 'Dynamic
        |host-string-upcase $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defwasm-import host-string-upcase (text) |host |string-upcase
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'String)
              :args $ [] 'String
        |main! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn main! () $ println (fibo 10)
          :examples $ []
          :schema $ :: 'Dynamic
        |reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn reload! $
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ []
        |sum-range $ %{} 'CodeEntry (:doc "|Sum 1..n via helper")
          :code $ quote
            defn sum-range (n) (sum-range-step 0 1 n)
          :examples $ []
          :schema $ :: 'Dynamic
        |sum-range-step $ %{} 'CodeEntry (:doc "|Sum step helper: sum-range-step(acc, i, n)")
          :code $ quote
            defn sum-range-step (acc i n)
              if (&> i n) acc $ recur (&+ acc i) (&+ i 1) n
          :examples $ []
          :schema $ :: 'Dynamic
        |sum-rest $ %{} 'CodeEntry (:doc "|variadic sum: a + b + rest...")
          :code $ quote
            defn sum-rest (a b & xs)
              sum-rest-list (&+ a b) xs
          :examples $ []
          :schema $ :: 'Dynamic
        |sum-rest-forward $ %{} 'CodeEntry (:doc "|forwards a rest list via &call-spread")
          :code $ quote
            defn sum-rest-forward (a b & xs) (sum-rest a b & xs)
          :examples $ []
          :schema $ :: 'Dynamic
        |sum-rest-list $ %{} 'CodeEntry (:doc "|helper: sums a list via recur")
          :code $ quote
            defn sum-rest-list (acc xs)
              if (&list:empty? xs) acc $ recur
                &+ acc $ &list:first xs
                &list:rest xs
          :examples $ []
          :schema $ :: 'Dynamic
        |test-abs $ %{} 'CodeEntry (:doc "|abs from calcit.core")
          :code $ quote
            defn test-abs (x) (abs x)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-bit-and $ %{} 'CodeEntry (:doc "|Bitwise AND")
          :code $ quote
            defn test-bit-and (a b) (bit-and a b)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-bit-not $ %{} 'CodeEntry (:doc "|Bitwise NOT")
          :code $ quote
            defn test-bit-not (a) (bit-not a)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-bit-or $ %{} 'CodeEntry (:doc "|Bitwise OR")
          :code $ quote
            defn test-bit-or (a b) (bit-or a b)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-bit-shl $ %{} 'CodeEntry (:doc "|Bitwise shift left")
          :code $ quote
            defn test-bit-shl (a b) (bit-shl a b)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-bit-shr $ %{} 'CodeEntry (:doc "|Bitwise shift right")
          :code $ quote
            defn test-bit-shr (a b) (bit-shr a b)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-bit-xor $ %{} 'CodeEntry (:doc "|Bitwise XOR")
          :code $ quote
            defn test-bit-xor (a b) (bit-xor a b)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-buf-list-doseq $ %{} 'CodeEntry (:doc "||buf-list: use doseq to push 4 items, count=4")
          :code $ quote
            defn test-buf-list-doseq () $ let
                buf $ &buf-list:new
              &doseq
                n $ [] 1 2 3 4
                &buf-list:push buf n
              &buf-list:count buf
          :examples $ []
          :schema $ :: 'Dynamic
        |test-buf-list-each $ %{} 'CodeEntry (:doc "||buf-list: use each to push 3 items, count=3")
          :code $ quote
            defn test-buf-list-each () $ let
                buf $ &buf-list:new
              each ([] 10 20 30)
                fn (x) (&buf-list:push buf x)
              &buf-list:count buf
          :examples $ []
          :schema $ :: 'Dynamic
        |test-buf-list-filter $ %{} 'CodeEntry (:doc "||buf-list: concat [1..5], filter even from to-list, count=2")
          :code $ quote
            defn test-buf-list-filter () $ let
                buf $ &buf-list:new
              &buf-list:concat buf $ [] 1 2 3 4 5
              &list:count $ filter (&buf-list:to-list buf)
                fn (x)
                  &= (&number:rem x 2) 0
          :examples $ []
          :schema $ :: 'Dynamic
        |test-buf-list-map $ %{} 'CodeEntry (:doc "||buf-list: concat 3 items, map to-list, count=3")
          :code $ quote
            defn test-buf-list-map () $ let
                buf $ &buf-list:new
              &buf-list:concat buf $ [] 1 2 3
              &list:count $ map (&buf-list:to-list buf)
                fn (x) (&* x 2)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-buf-list-push $ %{} 'CodeEntry (:doc "||buf-list push 3 items, count=3")
          :code $ quote
            defn test-buf-list-push () $ let
                buf $ &buf-list:new
              &buf-list:push buf 10
              &buf-list:push buf 20
              &buf-list:push buf 30
              &buf-list:count buf
          :examples $ []
          :schema $ :: 'Dynamic
        |test-buf-list-to-list $ %{} 'CodeEntry (:doc "||buf-list concat [1,2,3] then to-list, count=3")
          :code $ quote
            defn test-buf-list-to-list () $ let
                buf $ &buf-list:new
                items $ [] 1 2 3
              &buf-list:concat buf items
              &list:count $ &buf-list:to-list buf
          :examples $ []
          :schema $ :: 'Dynamic
        |test-call-spread-rest $ %{} 'CodeEntry (:doc "|rest list forwarding via &call-spread")
          :code $ quote
            defn test-call-spread-rest () $ sum-rest-forward 1 2 3 4 5
          :examples $ []
          :schema $ :: 'Dynamic
        |test-ceil $ %{} 'CodeEntry (:doc "|ceil function")
          :code $ quote
            defn test-ceil (x) (ceil x)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-compare $ %{} 'CodeEntry (:doc "|comparison chain")
          :code $ quote
            defn test-compare (a b)
              if (&< a b) -1 $ if (&> a b) 1 0
          :examples $ []
          :schema $ :: 'Dynamic
        |test-cos $ %{} 'CodeEntry (:doc "|cos via host import")
          :code $ quote
            defn test-cos (x) (cos x)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-cross-ns $ %{} 'CodeEntry (:doc "|Cross-namespace function call")
          :code $ quote
            defn test-cross-ns (a b) (helper/add-and-double a b)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-display-by-bin $ %{} 'CodeEntry (:doc "|17 in binary = 0b10001, length 7")
          :code $ quote
            defn test-display-by-bin () $ &str:count (&number:display-by 17 2)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-display-by-hex $ %{} 'CodeEntry (:doc "|17 in hex = 0x11, length 4")
          :code $ quote
            defn test-display-by-hex () $ &str:count (&number:display-by 17 16)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-find-found $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn test-find-found () $ option:unwrap-or
              find ([] 1 2 3)
                fn (x) (> x 1)
              , -1
          :examples $ []
          :schema $ :: 'Dynamic
        |test-find-index-found $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn test-find-index-found () $ option:unwrap-or
              find-index ([] 1 2 3)
                fn (x) (> x 1)
              , -1
          :examples $ []
          :schema $ :: 'Dynamic
        |test-find-index-not-found $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn test-find-index-not-found () $ option:unwrap-or
              find-index ([] 1 2 3)
                fn (x) (> x 9)
              , -1
          :examples $ []
          :schema $ :: 'Dynamic
        |test-find-not-found $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn test-find-not-found () $ option:unwrap-or
              find ([] 1 2 3)
                fn (x) (> x 9)
              , -1
          :examples $ []
          :schema $ :: 'Dynamic
        |test-floor $ %{} 'CodeEntry (:doc "|floor function")
          :code $ quote
            defn test-floor (x) (floor x)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-gte $ %{} 'CodeEntry (:doc |greater-than-or-equal)
          :code $ quote
            defn test-gte (a b)
              if (&> a b) 1 $ if (&= a b) 1 0
          :examples $ []
          :schema $ :: 'Dynamic
        |test-hash-number $ %{} 'CodeEntry (:doc "|hash on number returns stable non-zero value")
          :code $ quote
            defn test-hash-number () $ if
              &> (&hash 42) 0
              , 1 0
          :examples $ []
          :schema $ :: 'Dynamic
        |test-let-chain $ %{} 'CodeEntry (:doc "|chained let bindings")
          :code $ quote
            defn test-let-chain (x)
              &let
                a $ &* x x
                &let
                  b $ &+ a 1
                  &* b 2
          :examples $ []
          :schema $ :: 'Dynamic
        |test-list-append $ %{} 'CodeEntry (:doc "|append returns correct count and last elem")
          :code $ quote
            defn test-list-append () $ &let
              xs $ append ([] 10 20) 30
              &+ (&list:count xs) (&list:nth xs 2)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-list-assoc $ %{} 'CodeEntry (:doc "|assoc replaces element")
          :code $ quote
            defn test-list-assoc () $ &list:nth
              &list:assoc ([] 10 20 30) 1 99
              , 1
          :examples $ []
          :schema $ :: 'Dynamic
        |test-list-assoc-after $ %{} 'CodeEntry (:doc "|assoc-after inserts element after index")
          :code $ quote
            defn test-list-assoc-after () $ &let
              xs $ &list:assoc-after ([] 10 20 30) 0 99
              &+ (&list:count xs) (&list:nth xs 1)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-list-assoc-before $ %{} 'CodeEntry (:doc "|assoc-before inserts element before index")
          :code $ quote
            defn test-list-assoc-before () $ &let
              xs $ &list:assoc-before ([] 10 20 30) 1 99
              &+ (&list:count xs) (&list:nth xs 1)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-list-butlast $ %{} 'CodeEntry (:doc "|butlast drops last element")
          :code $ quote
            defn test-list-butlast () $ &list:count
              butlast $ [] 10 20 30
          :examples $ []
          :schema $ :: 'Dynamic
        |test-list-butlast-empty $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn test-list-butlast-empty () $ &list:count
              butlast $ []
          :examples $ []
          :schema $ :: 'Dynamic
        |test-list-concat $ %{} 'CodeEntry (:doc "|concat two lists")
          :code $ quote
            defn test-list-concat () $ &let
              xs $ &list:concat ([] 10 20) ([] 30 40)
              &+ (&list:count xs) (&list:nth xs 3)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-list-contains $ %{} 'CodeEntry (:doc "|contains checks index bounds")
          :code $ quote
            defn test-list-contains () $ &let
              xs $ [] 10 20 30
              &+
                if (&list:contains? xs 2) 1 0
                if (&list:contains? xs 5) 10 0
          :examples $ []
          :schema $ :: 'Dynamic
        |test-list-contains-method $ %{} 'CodeEntry (:doc "|.contains? dispatches on list")
          :code $ quote
            defn test-list-contains-method () $ &+
              if
                .contains? ([] 10 20 30) 1
                , 1 0
              if
                .contains? ([] 10 20 30) 9
                , 10 0
          :examples $ []
          :schema $ :: 'Dynamic
        |test-list-count $ %{} 'CodeEntry (:doc "|list count")
          :code $ quote
            defn test-list-count () $ &list:count ([] 10 20 30)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-list-dissoc $ %{} 'CodeEntry (:doc "|dissoc removes element")
          :code $ quote
            defn test-list-dissoc () $ &let
              xs $ &list:dissoc ([] 10 20 30) 1
              &+ (&list:count xs) (&list:nth xs 1)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-list-empty-false $ %{} 'CodeEntry (:doc "|non-empty list not empty")
          :code $ quote
            defn test-list-empty-false () $ if
              &list:empty? $ [] 1
              , 1 0
          :examples $ []
          :schema $ :: 'Dynamic
        |test-list-empty-method $ %{} 'CodeEntry (:doc "|.empty returns an empty list")
          :code $ quote
            defn test-list-empty-method () $ count
              .empty $ [] 10 20 30
          :examples $ []
          :schema $ :: 'Dynamic
        |test-list-empty-true $ %{} 'CodeEntry (:doc "|empty list is empty")
          :code $ quote
            defn test-list-empty-true () $ if
              &list:empty? $ []
              , 1 0
          :examples $ []
          :schema $ :: 'Dynamic
        |test-list-empty?-method $ %{} 'CodeEntry (:doc "|.empty? uses generic method dispatch")
          :code $ quote
            defn test-list-empty?-method () $ if
              .empty? $ []
              , 1 0
          :examples $ []
          :schema $ :: 'Dynamic
        |test-list-first $ %{} 'CodeEntry (:doc "|list first element")
          :code $ quote
            defn test-list-first () $ &list:first ([] 42 99)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-list-first-generic $ %{} 'CodeEntry (:doc "|generic first() on list via invoke")
          :code $ quote
            defn test-list-first-generic () $ option:unwrap
              first $ [] 42 99
          :examples $ []
          :schema $ :: 'Dynamic
        |test-list-includes $ %{} 'CodeEntry (:doc "|includes checks value presence")
          :code $ quote
            defn test-list-includes () $ &+
              if
                &list:includes? ([] 10 20 30) 20
                , 1 0
              if
                &list:includes? ([] 10 20 30) 99
                , 10 0
          :examples $ []
          :schema $ :: 'Dynamic
        |test-list-includes-method $ %{} 'CodeEntry (:doc "|.includes? dispatches on list")
          :code $ quote
            defn test-list-includes-method () $ &+
              if
                .includes? ([] 10 20 30) 20
                , 1 0
              if
                .includes? ([] 10 20 30) 99
                , 10 0
          :examples $ []
          :schema $ :: 'Dynamic
        |test-list-max-empty $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn test-list-max-empty () $ option:unwrap-or
              .max $ []
              , -1
          :examples $ []
          :schema $ :: 'Dynamic
        |test-list-max-method $ %{} 'CodeEntry (:doc "|.max dispatches on list")
          :code $ quote
            defn test-list-max-method () $ option:unwrap-or
              .max $ [] 10 20 30 15
              , -1
          :examples $ []
          :schema $ :: 'Dynamic
        |test-list-min-method $ %{} 'CodeEntry (:doc "|.min dispatches on list")
          :code $ quote
            defn test-list-min-method () $ option:unwrap-or
              .min $ [] 10 20 30 15
              , -1
          :examples $ []
          :schema $ :: 'Dynamic
        |test-list-nth $ %{} 'CodeEntry (:doc "|list nth element")
          :code $ quote
            defn test-list-nth (i)
              &list:nth ([] 10 20 30 40) i
          :examples $ []
          :schema $ :: 'Dynamic
        |test-list-prepend $ %{} 'CodeEntry (:doc "|prepend returns correct first elem")
          :code $ quote
            defn test-list-prepend () $ &list:first
              prepend ([] 10 20) 5
          :examples $ []
          :schema $ :: 'Dynamic
        |test-list-rest-count $ %{} 'CodeEntry (:doc "|count of rest")
          :code $ quote
            defn test-list-rest-count () $ &list:count
              &list:rest $ [] 10 20 30
          :examples $ []
          :schema $ :: 'Dynamic
        |test-list-rest-empty $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn test-list-rest-empty () $ &list:count
              &list:rest $ []
          :examples $ []
          :schema $ :: 'Dynamic
        |test-list-rest-first $ %{} 'CodeEntry (:doc "|first of rest")
          :code $ quote
            defn test-list-rest-first () $ &list:first
              &list:rest $ [] 10 20 30
          :examples $ []
          :schema $ :: 'Dynamic
        |test-list-rest-generic-first $ %{} 'CodeEntry (:doc "|generic rest() on list via invoke")
          :code $ quote
            defn test-list-rest-generic-first () $ option:unwrap
              first $ rest ([] 10 20 30)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-list-reverse $ %{} 'CodeEntry (:doc "|reverse a list")
          :code $ quote
            defn test-list-reverse () $ &let
              xs $ &list:reverse ([] 10 20 30)
              &+ (&list:first xs) (&list:nth xs 2)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-list-slice $ %{} 'CodeEntry (:doc "|slice with start and end")
          :code $ quote
            defn test-list-slice () $ &let
              xs $ &list:slice ([] 10 20 30 40 50) 1 4
              &+ (&list:count xs) (&list:first xs)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-list-to-set $ %{} 'CodeEntry (:doc "|list to set deduplicates elements")
          :code $ quote
            defn test-list-to-set () $ &let
              s $ &list:to-set ([] 10 20 30 20 10)
              &set:count s
          :examples $ []
          :schema $ :: 'Dynamic
        |test-list?-false $ %{} 'CodeEntry (:doc "|list? on number returns false (0)")
          :code $ quote
            defn test-list?-false () $ if (list? 42) 1 0
          :examples $ []
          :schema $ :: 'Dynamic
        |test-list?-true $ %{} 'CodeEntry (:doc "|list? on a list returns true (1)")
          :code $ quote
            defn test-list?-true () $ if
              list? $ [] 1 2
              , 1 0
          :examples $ []
          :schema $ :: 'Dynamic
        |test-lte $ %{} 'CodeEntry (:doc |less-than-or-equal)
          :code $ quote
            defn test-lte (a b)
              if (&< a b) 1 $ if (&= a b) 1 0
          :examples $ []
          :schema $ :: 'Dynamic
        |test-map-assoc-new $ %{} 'CodeEntry (:doc "|assoc adds new key")
          :code $ quote
            defn test-map-assoc-new () $ &let
              m $ &map:assoc (&{} :a 1) :b 2
              &+ (&map:count m) (&map:get m :b)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-map-assoc-update $ %{} 'CodeEntry (:doc "|assoc updates existing key")
          :code $ quote
            defn test-map-assoc-update () $ &map:get
              &map:assoc (&{} :a 1 :b 2) :b 99
              , :b
          :examples $ []
          :schema $ :: 'Dynamic
        |test-map-bucket-update $ %{} 'CodeEntry (:doc "|update on collided numeric keys keeps lookup correct")
          :code $ quote
            defn test-map-bucket-update (a b)
              &let
                m $ &map:assoc (&{} a 10 b 20) b 99
                &+ (&map:get m a) (&map:get m b)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-map-common-keys $ %{} 'CodeEntry (:doc "|common-keys: keys in both a and b")
          :code $ quote
            defn test-map-common-keys () $ &set:count
              &map:common-keys (&{} :a 1 :b 2 :c 3) (&{} :b 10 :c 20 :d 30)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-map-contains $ %{} 'CodeEntry (:doc "|contains checks key presence")
          :code $ quote
            defn test-map-contains () $ &+
              if
                &map:contains? (&{} :a 1 :b 2) :a
                , 1 0
              if
                &map:contains? (&{} :a 1 :b 2) :z
                , 10 0
          :examples $ []
          :schema $ :: 'Dynamic
        |test-map-contains-method $ %{} 'CodeEntry (:doc "|.contains? dispatches on map")
          :code $ quote
            defn test-map-contains-method () $ &+
              if
                .contains? (&{} :a 1 :b 2) :a
                , 1 0
              if
                .contains? (&{} :a 1 :b 2) :z
                , 10 0
          :examples $ []
          :schema $ :: 'Dynamic
        |test-map-count $ %{} 'CodeEntry (:doc "|map count")
          :code $ quote
            defn test-map-count () $ &map:count (&{} :a 1 :b 2 :c 3)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-map-diff-keys $ %{} 'CodeEntry (:doc "|diff-keys: keys in a not in b")
          :code $ quote
            defn test-map-diff-keys () $ &set:count
              &map:diff-keys (&{} :a 1 :b 2 :c 3) (&{} :b 10)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-map-diff-new $ %{} 'CodeEntry (:doc "|diff-new: entries in b not in a")
          :code $ quote
            defn test-map-diff-new () $ &map:count
              &map:diff-new (&{} :a 1 :b 2) (&{} :b 3 :c 4 :d 5)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-map-dissoc $ %{} 'CodeEntry (:doc "|dissoc removes key")
          :code $ quote
            defn test-map-dissoc () $ &let
              m $ &map:dissoc (&{} :a 1 :b 2 :c 3) :b
              &+ (&map:count m) (&map:get m :c)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-map-empty-false $ %{} 'CodeEntry (:doc "|non-empty map not empty")
          :code $ quote
            defn test-map-empty-false () $ if
              &map:empty? $ &{} :a 1
              , 1 0
          :examples $ []
          :schema $ :: 'Dynamic
        |test-map-empty-method $ %{} 'CodeEntry (:doc "|.empty returns an empty map")
          :code $ quote
            defn test-map-empty-method () $ count
              .empty $ &{} :a 1 :b 2
          :examples $ []
          :schema $ :: 'Dynamic
        |test-map-empty-true $ %{} 'CodeEntry (:doc "|empty map is empty")
          :code $ quote
            defn test-map-empty-true () $ if
              &map:empty? $ &{}
              , 1 0
          :examples $ []
          :schema $ :: 'Dynamic
        |test-map-get $ %{} 'CodeEntry (:doc "|map get by key")
          :code $ quote
            defn test-map-get () $ &map:get (&{} :a 10 :b 20 :c 30) :b
          :examples $ []
          :schema $ :: 'Dynamic
        |test-map-hash-index1 $ %{} 'CodeEntry (:doc "|second 5 bits of number hash")
          :code $ quote
            defn test-map-hash-index1 (n)
              bit-and
                bit-shr (&hash n) 5
                , 31
          :examples $ []
          :schema $ :: 'Dynamic
        |test-map-hash-value $ %{} 'CodeEntry (:doc "|raw hash for numeric key")
          :code $ quote
            defn test-map-hash-value (n) (&hash n)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-map-includes $ %{} 'CodeEntry (:doc "|map includes checks value")
          :code $ quote
            defn test-map-includes () $ &+
              if
                &map:includes? (&{} :a 10 :b 20) 20
                , 1 0
              if
                &map:includes? (&{} :a 10 :b 20) 99
                , 10 0
          :examples $ []
          :schema $ :: 'Dynamic
        |test-map-includes-method $ %{} 'CodeEntry (:doc "|.includes? dispatches on map")
          :code $ quote
            defn test-map-includes-method () $ &+
              if
                .includes? (&{} :a 10 :b 20) 20
                , 1 0
              if
                .includes? (&{} :a 10 :b 20) 99
                , 10 0
          :examples $ []
          :schema $ :: 'Dynamic
        |test-map-merge $ %{} 'CodeEntry (:doc "|merge two maps, b overrides a")
          :code $ quote
            defn test-map-merge () $ &map:count
              &merge (&{} :a 1 :b 2) (&{} :b 3 :c 4)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-map-merge-value $ %{} 'CodeEntry (:doc "|merge override check via get")
          :code $ quote
            defn test-map-merge-value () $ &map:get
              &merge (&{} :a 1 :b 2) (&{} :b 99)
              , :b
          :examples $ []
          :schema $ :: 'Dynamic
        |test-map-two-keys-sum $ %{} 'CodeEntry (:doc "|sum lookups for two numeric keys")
          :code $ quote
            defn test-map-two-keys-sum (a b)
              &let
                m $ &{} a 10 b 20
                &+ (&map:get m a) (&map:get m b)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-map?-true $ %{} 'CodeEntry (:doc "|map? on map returns true (1)")
          :code $ quote
            defn test-map?-true () $ if
              map? $ &{} :a 1
              , 1 0
          :examples $ []
          :schema $ :: 'Dynamic
        |test-match-sub $ %{} 'CodeEntry (:doc "|Match on second variant")
          :code $ quote
            defn test-match-sub (x y)
              &let
                t $ :: :sub x y
                match t
                  (:add a b) (&+ a b)
                  (:sub a b) (&- a b)
                  _ 0
          :examples $ []
          :schema $ :: 'Dynamic
        |test-match-tag $ %{} 'CodeEntry (:doc "|Match on tuple tag")
          :code $ quote
            defn test-match-tag (x y)
              &let
                t $ :: :add x y
                match t
                  (:add a b) (&+ a b)
                  (:sub a b) (&- a b)
                  _ 0
          :examples $ []
          :schema $ :: 'Dynamic
        |test-match-wildcard $ %{} 'CodeEntry (:doc "|Match falls to wildcard")
          :code $ quote
            defn test-match-wildcard () $ &let
              t $ :: :unknown 99
              match t
                (:add a b) (&+ a b)
                _ -1
          :examples $ []
          :schema $ :: 'Dynamic
        |test-max $ %{} 'CodeEntry (:doc "|max of two numbers")
          :code $ quote
            defn test-max (a b)
              if (&> a b) a b
          :examples $ []
          :schema $ :: 'Dynamic
        |test-min $ %{} 'CodeEntry (:doc "|min of two numbers")
          :code $ quote
            defn test-min (a b)
              if (&< a b) a b
          :examples $ []
          :schema $ :: 'Dynamic
        |test-negate $ %{} 'CodeEntry (:doc "|negate from calcit.core")
          :code $ quote
            defn test-negate (x) (negate x)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-not $ %{} 'CodeEntry (:doc "|not operation")
          :code $ quote
            defn test-not (x) (not x)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-number-compare-method $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn test-number-compare-method () $ .compare 1 2
          :examples $ []
          :schema $ :: 'Dynamic
        |test-number?-true $ %{} 'CodeEntry (:doc "|number? on number returns true (1)")
          :code $ quote
            defn test-number?-true () $ if (number? 42) 1 0
          :examples $ []
          :schema $ :: 'Dynamic
        |test-option-unwrap-or $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn test-option-unwrap-or () $ option:unwrap-or (%none) 7
          :examples $ []
          :schema $ :: 'Dynamic
        |test-pow $ %{} 'CodeEntry (:doc "|pow via host import")
          :code $ quote
            defn test-pow (base exp) (pow base exp)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-println $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn test-println () do (println 42) 1
          :examples $ []
          :schema $ :: 'Dynamic
        |test-range $ %{} 'CodeEntry (:doc "|range creates list of numbers")
          :code $ quote
            defn test-range () $ &list:count (range 5)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-range-sum $ %{} 'CodeEntry (:doc "|range 5 first+last: 0+4=4")
          :code $ quote
            defn test-range-sum () $ &let
              xs $ range 5
              &+ (&list:nth xs 0) (&list:nth xs 4)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-range-two-args $ %{} 'CodeEntry (:doc "|range 2 5 creates 3 elements")
          :code $ quote
            defn test-range-two-args () $ &list:count (range 2 5)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-record-field-tag $ %{} 'CodeEntry (:doc "|record field-tag resolves by index")
          :code $ quote
            defn test-record-field-tag () $ &let
              point $ %{} Point (:x 1) (:y 2)
              if
                &= (&struct:field-tag point 0) :x
                , 1 0
          :examples $ []
          :schema $ :: 'Dynamic
        |test-record-get-name $ %{} 'CodeEntry (:doc "|record get-name returns struct tag")
          :code $ quote
            defn test-record-get-name () $ &let
              point $ %{} Point (:x 1) (:y 2)
              if
                &= (&struct:get-name point) :Point
                , 1 0
          :examples $ []
          :schema $ :: 'Dynamic
        |test-record-matches-true $ %{} 'CodeEntry (:doc "|record:matches? returns true for same type")
          :code $ quote
            defn test-record-matches-true () $ &let
              a $ %{} Point (:x 1) (:y 2)
              &let
                b $ %{} Point (:x 3) (:y 4)
                if (&struct:matches? a b) 1 0
          :examples $ []
          :schema $ :: 'Dynamic
        |test-record-struct-eq $ %{} 'CodeEntry (:doc "|record struct equals source struct")
          :code $ quote
            defn test-record-struct-eq () $ &let
              point $ %{} Point (:x 1) (:y 2)
              if
                &= (&struct:definition point) Point
                , 1 0
          :examples $ []
          :schema $ :: 'Dynamic
        |test-record-sum $ %{} 'CodeEntry (:doc "|Record create + field access")
          :code $ quote
            defn test-record-sum (x y)
              &let
                p $ %{} Point (:x x) (:y y)
                &+ (&struct:nth p 0 :x) (&struct:nth p 1 :y)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-record-to-map $ %{} 'CodeEntry (:doc "|record to-map exposes field values by tag")
          :code $ quote
            defn test-record-to-map () $ &let
              point $ %{} Point (:x 1) (:y 2)
              &let
                m $ &struct:to-map point
                &+ (&map:get m :x) (&map:get m :y)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-rem $ %{} 'CodeEntry (:doc |remainder)
          :code $ quote
            defn test-rem (a b) (&number:rem a b)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-rest-count $ %{} 'CodeEntry (:doc "|rest args count: 3 extras")
          :code $ quote
            defn test-rest-count () $ &list:count (collect-rest 1 2 3 4)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-rest-empty $ %{} 'CodeEntry (:doc "|rest args with no extras: 10+20 = 30")
          :code $ quote
            defn test-rest-empty () $ sum-rest 10 20
          :examples $ []
          :schema $ :: 'Dynamic
        |test-rest-sum $ %{} 'CodeEntry (:doc "|rest args: 1+2+3+4+5 = 15")
          :code $ quote
            defn test-rest-sum () $ sum-rest 1 2 3 4 5
          :examples $ []
          :schema $ :: 'Dynamic
        |test-result-unwrap-or $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn test-result-unwrap-or () $ result:unwrap-or (%err 3) 7
          :examples $ []
          :schema $ :: 'Dynamic
        |test-round $ %{} 'CodeEntry (:doc "|round function")
          :code $ quote
            defn test-round (x) (round x)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-set-contains-method $ %{} 'CodeEntry (:doc "|.contains? dispatches on set")
          :code $ quote
            defn test-set-contains-method () $ &+
              if
                .contains? (#{} 10 20 30) 20
                , 1 0
              if
                .contains? (#{} 10 20 30) 99
                , 10 0
          :examples $ []
          :schema $ :: 'Dynamic
        |test-set-count $ %{} 'CodeEntry (:doc "|set count")
          :code $ quote
            defn test-set-count () $ &set:count (#{} 10 20 30)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-set-difference $ %{} 'CodeEntry (:doc "|difference removes elements in second set")
          :code $ quote
            defn test-set-difference () $ &set:count
              &difference (#{} 10 20 30 40) (#{} 20 40)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-set-difference-empty $ %{} 'CodeEntry (:doc "|difference with disjoint sets keeps all")
          :code $ quote
            defn test-set-difference-empty () $ &set:count
              &difference (#{} 10 20) (#{} 30 40)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-set-empty $ %{} 'CodeEntry (:doc "|empty set")
          :code $ quote
            defn test-set-empty () $ &+
              if
                &set:empty? $ #{}
                , 1 0
              if
                &set:empty? $ #{} 1
                , 10 0
          :examples $ []
          :schema $ :: 'Dynamic
        |test-set-empty-method $ %{} 'CodeEntry (:doc "|.empty returns an empty set")
          :code $ quote
            defn test-set-empty-method () $ count
              .empty $ #{} 10 20 30
          :examples $ []
          :schema $ :: 'Dynamic
        |test-set-exclude $ %{} 'CodeEntry (:doc "|exclude removes element")
          :code $ quote
            defn test-set-exclude () $ &set:count
              &exclude (#{} 10 20 30) 20
          :examples $ []
          :schema $ :: 'Dynamic
        |test-set-include $ %{} 'CodeEntry (:doc "|include adds element")
          :code $ quote
            defn test-set-include () $ &set:count
              &include (#{} 10 20) 30
          :examples $ []
          :schema $ :: 'Dynamic
        |test-set-includes $ %{} 'CodeEntry (:doc "|set includes value")
          :code $ quote
            defn test-set-includes () $ &+
              if
                &set:includes? (#{} 10 20 30) 20
                , 1 0
              if
                &set:includes? (#{} 10 20 30) 99
                , 10 0
          :examples $ []
          :schema $ :: 'Dynamic
        |test-set-includes-method $ %{} 'CodeEntry (:doc "|.includes? dispatches on set")
          :code $ quote
            defn test-set-includes-method () $ &+
              if
                .includes? (#{} 10 20 30) 20
                , 1 0
              if
                .includes? (#{} 10 20 30) 99
                , 10 0
          :examples $ []
          :schema $ :: 'Dynamic
        |test-set-max-method $ %{} 'CodeEntry (:doc "|.max dispatches on set")
          :code $ quote
            defn test-set-max-method () $ option:unwrap-or
              .max $ #{} 10 20 30 15
              , -1
          :examples $ []
          :schema $ :: 'Dynamic
        |test-set-min-method $ %{} 'CodeEntry (:doc "|.min dispatches on set")
          :code $ quote
            defn test-set-min-method () $ option:unwrap-or
              .min $ #{} 10 20 30 15
              , -1
          :examples $ []
          :schema $ :: 'Dynamic
        |test-set-union $ %{} 'CodeEntry (:doc "|union merges two sets")
          :code $ quote
            defn test-set-union () $ &set:count
              &union (#{} 10 20) (#{} 20 30 40)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-set-union-same $ %{} 'CodeEntry (:doc "|union of identical sets")
          :code $ quote
            defn test-set-union-same () $ &set:count
              &union (#{} 10 20 30) (#{} 10 20 30)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-sin $ %{} 'CodeEntry (:doc "|sin via host import")
          :code $ quote
            defn test-sin (x) (sin x)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-sqrt $ %{} 'CodeEntry (:doc "|sqrt function")
          :code $ quote
            defn test-sqrt (x) (sqrt x)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-str-compare-eq $ %{} 'CodeEntry (:doc "|compare equal strings = 0")
          :code $ quote
            defn test-str-compare-eq () $ &str:compare |abc |abc
          :examples $ []
          :schema $ :: 'Dynamic
        |test-str-compare-gt $ %{} 'CodeEntry (:doc "|compare abd > abc = 1")
          :code $ quote
            defn test-str-compare-gt () $ &str:compare |abd |abc
          :examples $ []
          :schema $ :: 'Dynamic
        |test-str-compare-lt $ %{} 'CodeEntry (:doc "|compare abc < abd = -1")
          :code $ quote
            defn test-str-compare-lt () $ &str:compare |abc |abd
          :examples $ []
          :schema $ :: 'Dynamic
        |test-str-concat $ %{} 'CodeEntry (:doc "|concat two strings and return byte count")
          :code $ quote
            defn test-str-concat () $ &str:count (&str:concat |foo |bar)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-str-contains-false $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn test-str-contains-false () $ &str:contains? |hello 10
          :examples $ []
          :schema $ :: 'Dynamic
        |test-str-contains-true $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn test-str-contains-true () $ &str:contains? |hello 1
          :examples $ []
          :schema $ :: 'Dynamic
        |test-str-count $ %{} 'CodeEntry (:doc "|string byte length")
          :code $ quote
            defn test-str-count () $ &str:count |hello
          :examples $ []
          :schema $ :: 'Dynamic
        |test-str-empty-false $ %{} 'CodeEntry (:doc "|non-empty string has non-zero count")
          :code $ quote
            defn test-str-empty-false () $ &= (&str:count |hi) 0
          :examples $ []
          :schema $ :: 'Dynamic
        |test-str-empty-true $ %{} 'CodeEntry (:doc "|rest of 1-char string has 0 bytes")
          :code $ quote
            defn test-str-empty-true () $ &=
              &str:count $ &str:rest |a
              , 0
          :examples $ []
          :schema $ :: 'Dynamic
        |test-str-escape $ %{} 'CodeEntry (:doc "|escape special chars")
          :code $ quote
            defn test-str-escape () $ &str:count (&str:escape |hello)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-str-find-index-found $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn test-str-find-index-found () $ option:unwrap-or (.find-index |hello |ell) -1
          :examples $ []
          :schema $ :: 'Dynamic
        |test-str-find-index-not-found $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn test-str-find-index-not-found () $ option:unwrap-or (.find-index |hello |xyz) -1
          :examples $ []
          :schema $ :: 'Dynamic
        |test-str-first $ %{} 'CodeEntry (:doc "|first byte of hello = 104 (h)")
          :code $ quote
            defn test-str-first () $ &str:first |hello
          :examples $ []
          :schema $ :: 'Dynamic
        |test-str-includes-false $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn test-str-includes-false () $ &str:includes? |hello |xyz
          :examples $ []
          :schema $ :: 'Dynamic
        |test-str-includes-true $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn test-str-includes-true () $ &str:includes? |hello |ell
          :examples $ []
          :schema $ :: 'Dynamic
        |test-str-nth $ %{} 'CodeEntry (:doc "|nth character at index 1 of hello is e")
          :code $ quote
            defn test-str-nth () $ if
              = (&str:nth |hello 1) |e
              , 1 0
          :examples $ []
          :schema $ :: 'Dynamic
        |test-str-pad-left $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn test-str-pad-left () $ &str:count (&str:pad-left |hi 5 |-)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-str-pad-right $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn test-str-pad-right () $ &str:count (&str:pad-right |hi 5 |-)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-str-rest $ %{} 'CodeEntry (:doc "|rest of hello has 4 bytes")
          :code $ quote
            defn test-str-rest () $ &str:count (&str:rest |hello)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-str-slice $ %{} 'CodeEntry (:doc "|slice bytes 1..4 from abcde = 3 bytes (bcd)")
          :code $ quote
            defn test-str-slice () $ &str:count (&str:slice |abcde 1 4)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-string-compare-method $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn test-string-compare-method () $ .compare |abc |abd
          :examples $ []
          :schema $ :: 'Dynamic
        |test-tag-eq $ %{} 'CodeEntry (:doc "|Tag equality — same tags")
          :code $ quote
            defn test-tag-eq () $ if (&= :ok :ok) 1 0
          :examples $ []
          :schema $ :: 'Dynamic
        |test-tag-neq $ %{} 'CodeEntry (:doc "|Tag inequality — different tags")
          :code $ quote
            defn test-tag-neq () $ if (&= :ok :err) 1 0
          :examples $ []
          :schema $ :: 'Dynamic
        |test-to-pairs $ %{} 'CodeEntry (:doc "|to-pairs count")
          :code $ quote
            defn test-to-pairs () $ &let
              ps $ to-pairs (&{} :a 1 :b 2)
              &+ (&list:count ps)
                &list:count $ &list:first ps
          :examples $ []
          :schema $ :: 'Dynamic
        |test-tuple-assoc $ %{} 'CodeEntry (:doc "|Tuple assoc updates payload by index")
          :code $ quote
            defn test-tuple-assoc () $ &let
              t $ &enum:assoc (:: :pair 10 20) 1 9
              &+ (&enum:nth t 1) (&enum:nth t 2)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-tuple-count $ %{} 'CodeEntry (:doc "|Tuple count returns payload count")
          :code $ quote
            defn test-tuple-count () $ &let
              t $ :: :pair 10 20
              &enum:count t
          :examples $ []
          :schema $ :: 'Dynamic
        |test-tuple-sum $ %{} 'CodeEntry (:doc "|Tuple create + nth access: idx 1 and 2 are payloads")
          :code $ quote
            defn test-tuple-sum () $ &let
              t $ :: :pair 10 20
              &+ (&enum:nth t 1) (&enum:nth t 2)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-type-of-list $ %{} 'CodeEntry (:doc "|type-of list == :list tag")
          :code $ quote
            defn test-type-of-list () $ if
              &=
                type-of $ [] 1 2 3
                , :list
              , 1 0
          :examples $ []
          :schema $ :: 'Dynamic
        |test-type-of-map $ %{} 'CodeEntry (:doc "|type-of map == :map tag")
          :code $ quote
            defn test-type-of-map () $ if
              &=
                type-of $ &{} :a 1
                , :map
              , 1 0
          :examples $ []
          :schema $ :: 'Dynamic
        |test-type-of-number $ %{} 'CodeEntry (:doc "|type-of number == :number tag")
          :code $ quote
            defn test-type-of-number () $ if
              &= (type-of 42) :number
              , 1 0
          :examples $ []
          :schema $ :: 'Dynamic
        |test-type-of-set $ %{} 'CodeEntry (:doc "|type-of set == :set tag")
          :code $ quote
            defn test-type-of-set () $ if
              &=
                type-of $ #{} 1 2
                , :set
              , 1 0
          :examples $ []
          :schema $ :: 'Dynamic
        |test-type-of-tuple $ %{} 'CodeEntry (:doc "|type-of tuple == :tuple tag")
          :code $ quote
            defn test-type-of-tuple () $ if
              &=
                type-of $ :: :Pair 1 2
                , :enum
              , 1 0
          :examples $ []
          :schema $ :: 'Dynamic
        |wasm-ffi-add $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defwasm-export wasm-ffi-add (a b) (&+ a b)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'Number 'Number
        |wasm-ffi-upcase $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defwasm-export wasm-ffi-upcase (text) (host-string-upcase text)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'String)
              :args $ [] 'String
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote
          ns test-wasm.main $ :require (test-wasm.helper :as helper)

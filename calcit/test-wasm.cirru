
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |test-wasm
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'test-wasm.main/main!) (:mode :native) (:reload-fn 'test-wasm.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'test-wasm.helper $ %{} 'FileEntry
      :defs $ {} $ 'add-and-double
        %{} 'CodeEntry (:doc "|Helper: add two numbers and double")
          :code $ quote $ defn add-and-double (a b)
            &* (&+ a b) 2
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number 'Number
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns test-wasm.helper
    'test-wasm.main $ %{} 'FileEntry
      :defs $ {}
        'Point $ %{} 'CodeEntry
          :doc "|Struct definition (via legacy defrecord) for WASM test"
          :code $ quote $ defrecord Point :x :y
          :examples $ []
          :schema $ :: 'StructDef
        'add-two $ %{} 'CodeEntry (:doc "|Simple addition")
          :code $ quote $ defwasm-export add-two (a b) (&+ a b)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number 'Number
        'collatz-steps $ %{} 'CodeEntry (:doc "|Collatz conjecture step counter")
          :code $ quote $ defwasm-export collatz-steps (n)
            if (&< n 2) 0 $ if
              &= (&number:rem n 2) 0
              &+ 1 $ collatz-steps $ &/ n 2
              &+ 1 $ collatz-steps $ &+ (&* 3 n) 1
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number
        'collect-rest $ %{} 'CodeEntry (:doc "|returns rest list unchanged")
          :code $ quote $ defwasm-export collect-rest (a & xs) xs
          :examples $ []
          :schema $ :: 'Fn $ {} (:rest 'Number)
            :args $ [] 'Number
            :return $ :: 'List 'Number
        'compare-wasm-ascending $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export compare-wasm-ascending (a b) (&- a b)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number 'Number
        'factorial $ %{} 'CodeEntry (:doc "|Factorial — recursive")
          :code $ quote $ defwasm-export factorial (n)
            if (&< n 2) 1 $ &* n $ factorial (&- n 1)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number
        'fibo $ %{} 'CodeEntry (:doc "|Fibonacci — recursive")
          :code $ quote $ defwasm-export fibo (n)
            if (&< n 2) 1 $ &+
              fibo $ &- n 1
              fibo $ &- n 2
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number
        'gcd $ %{} 'CodeEntry (:doc "|Greatest common divisor")
          :code $ quote $ defwasm-export gcd (a b)
            if (&= b 0) a $ recur b $ &number:rem a b
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number 'Number
        'host-string-upcase $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-import host-string-upcase (text) |host |string-upcase
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ [] 'String
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export main! ()
            println $ fibo 10
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export reload! ()
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'sum-range $ %{} 'CodeEntry (:doc "|Sum 1..n via helper")
          :code $ quote $ defwasm-export sum-range (n) (sum-range-step 0 1 n)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number
        'sum-range-step $ %{} 'CodeEntry (:doc "|Sum step helper: sum-range-step(acc, i, n)")
          :code $ quote $ defwasm-export sum-range-step (acc i n)
            if (&> i n) acc $ recur (&+ acc i) (&+ i 1) n
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number 'Number 'Number
        'sum-rest $ %{} 'CodeEntry (:doc "|variadic sum: a + b + rest...")
          :code $ quote $ defwasm-export sum-rest (a b & xs)
            sum-rest-list (&+ a b) xs
          :examples $ []
          :schema $ :: 'Fn $ {} (:rest 'Number) (:return 'Number)
            :args $ [] 'Number 'Number
        'sum-rest-forward $ %{} 'CodeEntry (:doc "|forwards a rest list via &call-spread")
          :code $ quote $ defwasm-export sum-rest-forward (a b & xs) (sum-rest a b & xs)
          :examples $ []
          :schema $ :: 'Fn $ {} (:rest 'Number) (:return 'Number)
            :args $ [] 'Number 'Number
        'sum-rest-list $ %{} 'CodeEntry (:doc "|helper: sums a list via recur")
          :code $ quote $ defwasm-export sum-rest-list (acc xs)
            if (&list:empty? xs) acc $ recur
              &+ acc $ &list:first xs
              &list:rest xs
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number 'Number
        'test-abs $ %{} 'CodeEntry (:doc "|abs from calcit.core")
          :code $ quote $ defwasm-export test-abs (x) (abs x)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number
        'test-bit-and $ %{} 'CodeEntry (:doc "|Bitwise AND")
          :code $ quote $ defwasm-export test-bit-and (a b) (bit-and a b)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number 'Number
        'test-bit-not $ %{} 'CodeEntry (:doc "|Bitwise NOT")
          :code $ quote $ defwasm-export test-bit-not (a) (bit-not a)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number
        'test-bit-or $ %{} 'CodeEntry (:doc "|Bitwise OR")
          :code $ quote $ defwasm-export test-bit-or (a b) (bit-or a b)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number 'Number
        'test-bit-shl $ %{} 'CodeEntry (:doc "|Bitwise shift left")
          :code $ quote $ defwasm-export test-bit-shl (a b) (bit-shl a b)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number 'Number
        'test-bit-shr $ %{} 'CodeEntry (:doc "|Bitwise shift right")
          :code $ quote $ defwasm-export test-bit-shr (a b) (bit-shr a b)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number 'Number
        'test-bit-xor $ %{} 'CodeEntry (:doc "|Bitwise XOR")
          :code $ quote $ defwasm-export test-bit-xor (a b) (bit-xor a b)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number 'Number
        'test-buf-list-doseq $ %{} 'CodeEntry (:doc "||buf-list: use doseq to push 4 items, count=4")
          :code $ quote $ defwasm-export test-buf-list-doseq ()
            let
                buf $ &buf-list:new
              &doseq
                n $ [] 1 2 3 4
                &buf-list:push buf n
              &buf-list:count buf
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-buf-list-each $ %{} 'CodeEntry (:doc "||buf-list: use each to push 3 items, count=3")
          :code $ quote $ defwasm-export test-buf-list-each ()
            let
                buf $ &buf-list:new
              each ([] 10 20 30)
                fn (x) (&buf-list:push buf x)
              &buf-list:count buf
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-buf-list-filter $ %{} 'CodeEntry
          :doc "||buf-list: concat [1..5], filter even from to-list, count=2"
          :code $ quote $ defwasm-export test-buf-list-filter ()
            let
                buf $ &buf-list:new
              &buf-list:concat buf $ [] 1 2 3 4 5
              &list:count $ filter (&buf-list:to-list buf)
                fn (x)
                  &= (&number:rem x 2) 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-buf-list-map $ %{} 'CodeEntry (:doc "||buf-list: concat 3 items, map to-list, count=3")
          :code $ quote $ defwasm-export test-buf-list-map ()
            let
                buf $ &buf-list:new
              &buf-list:concat buf $ [] 1 2 3
              &list:count $ map (&buf-list:to-list buf)
                fn (x) (&* x 2)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-buf-list-push $ %{} 'CodeEntry (:doc "||buf-list push 3 items, count=3")
          :code $ quote $ defwasm-export test-buf-list-push ()
            let
                buf $ &buf-list:new
              &buf-list:push buf 10
              &buf-list:push buf 20
              &buf-list:push buf 30
              &buf-list:count buf
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-buf-list-to-list $ %{} 'CodeEntry (:doc "||buf-list concat [1,2,3] then to-list, count=3")
          :code $ quote $ defwasm-export test-buf-list-to-list ()
            let
                buf $ &buf-list:new
                items $ [] 1 2 3
              &buf-list:concat buf items
              &list:count $ &buf-list:to-list buf
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-call-spread-rest $ %{} 'CodeEntry (:doc "|rest list forwarding via &call-spread")
          :code $ quote $ defwasm-export test-call-spread-rest () (sum-rest-forward 1 2 3 4 5)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-ceil $ %{} 'CodeEntry (:doc "|ceil function")
          :code $ quote $ defwasm-export test-ceil (x) (ceil x)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number
        'test-closure-capture-map $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export test-closure-capture-map ()
            let
                offset 10
                add-offset $ fn (x)
                  hint-fn $ {}
                    :args $ [] 'Number
                    :return 'Number
                  + x offset
              let
                  offset 100
                  values $ map ([] 1 2 3) add-offset
                if (&list:includes? values 11) 1 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |preserves-lexical-capture-through-map)
            :code $ quote $ assert= 1 (test-closure-capture-map)
            :tags $ #{} :core :unit :wasm
        'test-closure-map-indexed $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export test-closure-map-indexed ()
            let
                values $ map-indexed ([] 4 5)
                  fn (x idx)
                    hint-fn $ {}
                      :args $ [] 'Number 'Number
                      :return 'Number
                    + x idx
              if (&list:includes? values 6) 1 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |preserves-number-index-in-wasm)
            :code $ quote $ assert= 1 (test-closure-map-indexed)
            :tags $ #{} :core :unit :wasm
        'test-compare $ %{} 'CodeEntry (:doc "|comparison chain")
          :code $ quote $ defwasm-export test-compare (a b)
            if (&< a b) -1 $ if (&> a b) 1 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number 'Number
        'test-cos $ %{} 'CodeEntry (:doc "|cos via host import")
          :code $ quote $ defwasm-export test-cos (x) (cos x)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number
        'test-cross-ns $ %{} 'CodeEntry (:doc "|Cross-namespace function call")
          :code $ quote $ defwasm-export test-cross-ns (a b) (helper/add-and-double a b)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number 'Number
        'test-display-by-bin $ %{} 'CodeEntry (:doc "|17 in binary = 0b10001, length 7")
          :code $ quote $ defwasm-export test-display-by-bin ()
            &str:count $ &number:display-by 17 2
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-display-by-hex $ %{} 'CodeEntry (:doc "|17 in hex = 0x11, length 4")
          :code $ quote $ defwasm-export test-display-by-hex ()
            &str:count $ &number:display-by 17 16
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-enum-assoc $ %{} 'CodeEntry (:doc "|Enum assoc updates payload by index")
          :code $ quote $ defwasm-export test-enum-assoc ()
            &let
              t $ &enum:assoc (:: :pair 10 20) 1 9
              &+ (&enum:nth t 1) (&enum:nth t 2)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-enum-count $ %{} 'CodeEntry (:doc "|Enum count returns payload count")
          :code $ quote $ defwasm-export test-enum-count ()
            &let
              t $ :: :pair 10 20
              &enum:count t
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-enum-sum $ %{} 'CodeEntry
          :doc "|Enum create + nth access: idx 1 and 2 are payloads"
          :code $ quote $ defwasm-export test-enum-sum ()
            &let
              t $ :: :pair 10 20
              &+ (&enum:nth t 1) (&enum:nth t 2)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-filter-map-kv $ %{} 'CodeEntry
          :doc "|Typed filter-map-kv keeps two transformed entries and drops one."
          :code $ quote $ defwasm-export test-filter-map-kv ()
            let
                output $ filter-map-kv (&{} :a 1 :b 2 :c 3)
                  fn (k v)
                    if (&> v 1)
                      %:: MapEntryDecision :keep k $ &* v 10
                      %:: MapEntryDecision :drop
              &+ (&map:count output) (&map:get output :c)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-find-found $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export test-find-found ()
            option:unwrap-or
              find ([] 1 2 3)
                fn (x) (> x 1)
              , -1
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-find-index-found $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export test-find-index-found ()
            option:unwrap-or
              find-index ([] 1 2 3)
                fn (x) (> x 1)
              , -1
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-find-index-not-found $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export test-find-index-not-found ()
            option:unwrap-or
              find-index ([] 1 2 3)
                fn (x) (> x 9)
              , -1
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-find-not-found $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export test-find-not-found ()
            option:unwrap-or
              find ([] 1 2 3)
                fn (x) (> x 9)
              , -1
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-floor $ %{} 'CodeEntry (:doc "|floor function")
          :code $ quote $ defwasm-export test-floor (x) (floor x)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number
        'test-gte $ %{} 'CodeEntry (:doc |greater-than-or-equal)
          :code $ quote $ defwasm-export test-gte (a b)
            if (&> a b) 1 $ if (&= a b) 1 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number 'Number
        'test-hash-number $ %{} 'CodeEntry (:doc "|hash on number returns stable non-zero value")
          :code $ quote $ defwasm-export test-hash-number ()
            if
              &> (&hash 42) 0
              , 1 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-let-chain $ %{} 'CodeEntry (:doc "|chained let bindings")
          :code $ quote $ defwasm-export test-let-chain (x)
            &let
              a $ &* x x
              &let
                b $ &+ a 1
                &* b 2
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number
        'test-list-append $ %{} 'CodeEntry (:doc "|append returns correct count and last elem")
          :code $ quote $ defwasm-export test-list-append ()
            &let
              xs $ append ([] 10 20) 30
              &+ (&list:count xs) (&list:nth xs 2)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-list-assoc $ %{} 'CodeEntry (:doc "|assoc replaces element")
          :code $ quote $ defwasm-export test-list-assoc ()
            &list:nth
              &list:assoc ([] 10 20 30) 1 99
              , 1
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-list-assoc-after $ %{} 'CodeEntry (:doc "|assoc-after inserts element after index")
          :code $ quote $ defwasm-export test-list-assoc-after ()
            &let
              xs $ &list:assoc-after ([] 10 20 30) 0 99
              &+ (&list:count xs) (&list:nth xs 1)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-list-assoc-before $ %{} 'CodeEntry (:doc "|assoc-before inserts element before index")
          :code $ quote $ defwasm-export test-list-assoc-before ()
            &let
              xs $ &list:assoc-before ([] 10 20 30) 1 99
              &+ (&list:count xs) (&list:nth xs 1)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-list-butlast $ %{} 'CodeEntry (:doc "|butlast drops last element")
          :code $ quote $ defwasm-export test-list-butlast ()
            &list:count $ butlast $ [] 10 20 30
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-list-butlast-empty $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export test-list-butlast-empty ()
            &list:count $ butlast $ []
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-list-concat $ %{} 'CodeEntry (:doc "|concat two lists")
          :code $ quote $ defwasm-export test-list-concat ()
            &let
              xs $ &list:concat ([] 10 20) ([] 30 40)
              &+ (&list:count xs) (&list:nth xs 3)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-list-contains $ %{} 'CodeEntry (:doc "|contains checks index bounds")
          :code $ quote $ defwasm-export test-list-contains ()
            &let
              xs $ [] 10 20 30
              &+
                if (&list:contains? xs 2) 1 0
                if (&list:contains? xs 5) 10 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-list-contains-method $ %{} 'CodeEntry (:doc "|.contains? dispatches on list")
          :code $ quote $ defwasm-export test-list-contains-method ()
            &+
              if
                contains? ([] 10 20 30) 1
                , 1 0
              if
                contains? ([] 10 20 30) 9
                , 10 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-list-count $ %{} 'CodeEntry (:doc "|list count")
          :code $ quote $ defwasm-export test-list-count ()
            &list:count $ [] 10 20 30
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-list-dissoc $ %{} 'CodeEntry (:doc "|dissoc removes element")
          :code $ quote $ defwasm-export test-list-dissoc ()
            &let
              xs $ &list:dissoc ([] 10 20 30) 1
              &+ (&list:count xs) (&list:nth xs 1)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-list-empty-false $ %{} 'CodeEntry (:doc "|non-empty list not empty")
          :code $ quote $ defwasm-export test-list-empty-false ()
            if
              &list:empty? $ [] 1
              , 1 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-list-empty-method $ %{} 'CodeEntry (:doc "|.empty returns an empty list")
          :code $ quote $ defwasm-export test-list-empty-method ()
            count $ empty $ [] 10 20 30
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-list-empty-true $ %{} 'CodeEntry (:doc "|empty list is empty")
          :code $ quote $ defwasm-export test-list-empty-true ()
            if
              &list:empty? $ []
              , 1 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-list-empty?-method $ %{} 'CodeEntry (:doc "|.empty? uses generic method dispatch")
          :code $ quote $ defwasm-export test-list-empty?-method ()
            if
              empty? $ []
              , 1 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-list-first $ %{} 'CodeEntry (:doc "|list first element")
          :code $ quote $ defwasm-export test-list-first ()
            &list:first $ [] 42 99
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-list-first-generic $ %{} 'CodeEntry (:doc "|generic first() on list via invoke")
          :code $ quote $ defwasm-export test-list-first-generic ()
            option:unwrap $ first $ [] 42 99
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-list-includes $ %{} 'CodeEntry (:doc "|includes checks value presence")
          :code $ quote $ defwasm-export test-list-includes ()
            &+
              if
                &list:includes? ([] 10 20 30) 20
                , 1 0
              if
                &list:includes? ([] 10 20 30) 99
                , 10 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-list-includes-method $ %{} 'CodeEntry (:doc "|.includes? dispatches on list")
          :code $ quote $ defwasm-export test-list-includes-method ()
            &+
              if
                includes? ([] 10 20 30) 20
                , 1 0
              if
                includes? ([] 10 20 30) 99
                , 10 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-list-max-empty $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export test-list-max-empty ()
            option:unwrap-or
              max $ []
              , -1
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-list-max-method $ %{} 'CodeEntry (:doc "|.max dispatches on list")
          :code $ quote $ defwasm-export test-list-max-method ()
            option:unwrap-or
              max $ [] 10 20 30 15
              , -1
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-list-min-method $ %{} 'CodeEntry (:doc "|.min dispatches on list")
          :code $ quote $ defwasm-export test-list-min-method ()
            option:unwrap-or
              min $ [] 10 20 30 15
              , -1
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-list-nth $ %{} 'CodeEntry (:doc "|list nth element")
          :code $ quote $ defwasm-export test-list-nth (i)
            &list:nth ([] 10 20 30 40) i
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number
        'test-list-prepend $ %{} 'CodeEntry (:doc "|prepend returns correct first elem")
          :code $ quote $ defwasm-export test-list-prepend ()
            &list:first $ prepend ([] 10 20) 5
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-list-rest-count $ %{} 'CodeEntry (:doc "|count of rest")
          :code $ quote $ defwasm-export test-list-rest-count ()
            &list:count $ &list:rest $ [] 10 20 30
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-list-rest-empty $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export test-list-rest-empty ()
            &list:count $ &list:rest $ []
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-list-rest-first $ %{} 'CodeEntry (:doc "|first of rest")
          :code $ quote $ defwasm-export test-list-rest-first ()
            &list:first $ &list:rest $ [] 10 20 30
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-list-rest-generic-first $ %{} 'CodeEntry (:doc "|generic rest() on list via invoke")
          :code $ quote $ defwasm-export test-list-rest-generic-first ()
            option:unwrap $ first $ rest ([] 10 20 30)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-list-reverse $ %{} 'CodeEntry (:doc "|reverse a list")
          :code $ quote $ defwasm-export test-list-reverse ()
            &let
              xs $ &list:reverse $ [] 10 20 30
              &+ (&list:first xs) (&list:nth xs 2)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-list-slice $ %{} 'CodeEntry (:doc "|slice with start and end")
          :code $ quote $ defwasm-export test-list-slice ()
            &let
              xs $ &list:slice ([] 10 20 30 40 50) 1 4
              &+ (&list:count xs) (&list:first xs)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-list-sort-ascending $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export test-list-sort-ascending ()
            &let
              ys $ sort ([] 4 1 3 2) &-
              +
                * 10 $ &list:first ys
                &list:last ys
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-list-sort-descending $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export test-list-sort-descending ()
            &let
              ys $ &list:sort ([] 4 1 3 2)
                fn (a b) (- b a)
              +
                * 10 $ &list:first ys
                &list:last ys
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-list-sort-dynamic-callee $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export test-list-sort-dynamic-callee ()
            &let (comparator compare-wasm-ascending)
              &let
                ys $ sort ([] 4 1 3 2) comparator
                +
                  * 10 $ &list:first ys
                  &list:last ys
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-list-sort-input-immutable $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export test-list-sort-input-immutable ()
            &let
              xs $ [] 4 1 3 2
              &let
                ys $ sort xs $ fn (a b) (- a b)
                +
                  * 10 $ &list:first xs
                  &list:first ys
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-list-sort-stable $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export test-list-sort-stable ()
            &let
              xs $ [] ([] 2 20) ([] 1 10) ([] 2 21) ([] 1 11)
              &let
                ys $ sort xs $ fn (a b)
                  - (&list:nth a 0) (&list:nth b 0)
                +
                  * 1000 $ &list:nth (&list:nth ys 0) 1
                  * 100 $ &list:nth (&list:nth ys 1) 1
                  * 10 $ &list:nth (&list:nth ys 2) 1
                  &list:nth (&list:nth ys 3) 1
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-list-to-set $ %{} 'CodeEntry (:doc "|list to set deduplicates elements")
          :code $ quote $ defwasm-export test-list-to-set ()
            &let
              s $ &list:to-set $ [] 10 20 30 20 10
              &set:count s
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-list?-false $ %{} 'CodeEntry (:doc "|list? on number returns false (0)")
          :code $ quote $ defwasm-export test-list?-false ()
            if (list? 42) 1 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-list?-true $ %{} 'CodeEntry (:doc "|list? on a list returns true (1)")
          :code $ quote $ defwasm-export test-list?-true ()
            if
              list? $ [] 1 2
              , 1 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-lte $ %{} 'CodeEntry (:doc |less-than-or-equal)
          :code $ quote $ defwasm-export test-lte (a b)
            if (&< a b) 1 $ if (&= a b) 1 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number 'Number
        'test-map-assoc-new $ %{} 'CodeEntry (:doc "|assoc adds new key")
          :code $ quote $ defwasm-export test-map-assoc-new ()
            &let
              m $ &map:assoc (&{} :a 1) :b 2
              &+ (&map:count m) (&map:get m :b)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-map-assoc-update $ %{} 'CodeEntry (:doc "|assoc updates existing key")
          :code $ quote $ defwasm-export test-map-assoc-update ()
            &map:get
              &map:assoc (&{} :a 1 :b 2) :b 99
              , :b
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-map-bucket-update $ %{} 'CodeEntry
          :doc "|update on collided numeric keys keeps lookup correct"
          :code $ quote $ defwasm-export test-map-bucket-update (a b)
            &let
              m $ &map:assoc (&{} a 10 b 20) b 99
              &+ (&map:get m a) (&map:get m b)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number 'Number
        'test-map-common-keys $ %{} 'CodeEntry (:doc "|common-keys: keys in both a and b")
          :code $ quote $ defwasm-export test-map-common-keys ()
            &set:count $ &map:common-keys (&{} :a 1 :b 2 :c 3) (&{} :b 10 :c 20 :d 30)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-map-contains $ %{} 'CodeEntry (:doc "|contains checks key presence")
          :code $ quote $ defwasm-export test-map-contains ()
            &+
              if
                &map:contains? (&{} :a 1 :b 2) :a
                , 1 0
              if
                &map:contains? (&{} :a 1 :b 2) :z
                , 10 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-map-contains-method $ %{} 'CodeEntry (:doc "|.contains? dispatches on map")
          :code $ quote $ defwasm-export test-map-contains-method ()
            &+
              if
                contains? (&{} :a 1 :b 2) :a
                , 1 0
              if
                contains? (&{} :a 1 :b 2) :z
                , 10 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-map-count $ %{} 'CodeEntry (:doc "|map count")
          :code $ quote $ defwasm-export test-map-count ()
            &map:count $ &{} :a 1 :b 2 :c 3
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-map-diff-keys $ %{} 'CodeEntry (:doc "|diff-keys: keys in a not in b")
          :code $ quote $ defwasm-export test-map-diff-keys ()
            &set:count $ &map:diff-keys (&{} :a 1 :b 2 :c 3) (&{} :b 10)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-map-diff-new $ %{} 'CodeEntry (:doc "|diff-new: entries in b not in a")
          :code $ quote $ defwasm-export test-map-diff-new ()
            &map:count $ &map:diff-new (&{} :a 1 :b 2) (&{} :b 3 :c 4 :d 5)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-map-dissoc $ %{} 'CodeEntry (:doc "|dissoc removes key")
          :code $ quote $ defwasm-export test-map-dissoc ()
            &let
              m $ &map:dissoc (&{} :a 1 :b 2 :c 3) :b
              &+ (&map:count m) (&map:get m :c)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-map-empty-false $ %{} 'CodeEntry (:doc "|non-empty map not empty")
          :code $ quote $ defwasm-export test-map-empty-false ()
            if
              &map:empty? $ &{} :a 1
              , 1 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-map-empty-method $ %{} 'CodeEntry (:doc "|.empty returns an empty map")
          :code $ quote $ defwasm-export test-map-empty-method ()
            count $ empty $ &{} :a 1 :b 2
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-map-empty-true $ %{} 'CodeEntry (:doc "|empty map is empty")
          :code $ quote $ defwasm-export test-map-empty-true ()
            if
              &map:empty? $ &{}
              , 1 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-map-get $ %{} 'CodeEntry (:doc "|map get by key")
          :code $ quote $ defwasm-export test-map-get ()
            &map:get (&{} :a 10 :b 20 :c 30) :b
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-map-hash-index1 $ %{} 'CodeEntry (:doc "|second 5 bits of number hash")
          :code $ quote $ defwasm-export test-map-hash-index1 (n)
            bit-and
              bit-shr (&hash n) 5
              , 31
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number
        'test-map-hash-value $ %{} 'CodeEntry (:doc "|raw hash for numeric key")
          :code $ quote $ defwasm-export test-map-hash-value (n) (&hash n)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number
        'test-map-includes $ %{} 'CodeEntry (:doc "|map includes checks value")
          :code $ quote $ defwasm-export test-map-includes ()
            &+
              if
                &map:includes? (&{} :a 10 :b 20) 20
                , 1 0
              if
                &map:includes? (&{} :a 10 :b 20) 99
                , 10 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-map-includes-method $ %{} 'CodeEntry (:doc "|.includes? dispatches on map")
          :code $ quote $ defwasm-export test-map-includes-method ()
            &+
              if
                includes? (&{} :a 10 :b 20) 20
                , 1 0
              if
                includes? (&{} :a 10 :b 20) 99
                , 10 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-map-keys-method $ %{} 'CodeEntry
          :doc "|typed `.keys` lowers to `&map:keys` and returns Set<K> across WASM."
          :code $ quote $ defwasm-export test-map-keys-method ()
            &set:count $ keys $ &{} :a 1 :b 2
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-map-merge $ %{} 'CodeEntry (:doc "|merge two maps, b overrides a")
          :code $ quote $ defwasm-export test-map-merge ()
            &map:count $ &merge (&{} :a 1 :b 2) (&{} :b 3 :c 4)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-map-merge-value $ %{} 'CodeEntry (:doc "|merge override check via get")
          :code $ quote $ defwasm-export test-map-merge-value ()
            &map:get
              &merge (&{} :a 1 :b 2) (&{} :b 99)
              , :b
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-map-two-keys-sum $ %{} 'CodeEntry (:doc "|sum lookups for two numeric keys")
          :code $ quote $ defwasm-export test-map-two-keys-sum (a b)
            &let
              m $ &{} a 10 b 20
              &+ (&map:get m a) (&map:get m b)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number 'Number
        'test-map?-true $ %{} 'CodeEntry (:doc "|map? on map returns true (1)")
          :code $ quote $ defwasm-export test-map?-true ()
            if
              map? $ &{} :a 1
              , 1 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-match-sub $ %{} 'CodeEntry (:doc "|Match on second variant")
          :code $ quote $ defwasm-export test-match-sub (x y)
            &let
              t $ :: :sub x y
              match t
                (:add a b) (&+ a b)
                (:sub a b) (&- a b)
                _ 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number 'Number
        'test-match-tag $ %{} 'CodeEntry (:doc "|Match on enum tag")
          :code $ quote $ defwasm-export test-match-tag (x y)
            &let
              t $ :: :add x y
              match t
                (:add a b) (&+ a b)
                (:sub a b) (&- a b)
                _ 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number 'Number
        'test-match-wildcard $ %{} 'CodeEntry (:doc "|Match falls to wildcard")
          :code $ quote $ defwasm-export test-match-wildcard ()
            &let
              t $ :: :unknown 99
              match t
                (:add a b) (&+ a b)
                _ -1
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-max $ %{} 'CodeEntry (:doc "|max of two numbers")
          :code $ quote $ defwasm-export test-max (a b)
            if (&> a b) a b
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number 'Number
        'test-min $ %{} 'CodeEntry (:doc "|min of two numbers")
          :code $ quote $ defwasm-export test-min (a b)
            if (&< a b) a b
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number 'Number
        'test-negate $ %{} 'CodeEntry (:doc "|negate from calcit.core")
          :code $ quote $ defwasm-export test-negate (x) (negate x)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number
        'test-not $ %{} 'CodeEntry (:doc "|not operation")
          :code $ quote $ defwasm-export test-not (x) (not x)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number
        'test-number-compare-method $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export test-number-compare-method () (&compare 1 2)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-number?-true $ %{} 'CodeEntry (:doc "|number? on number returns true (1)")
          :code $ quote $ defwasm-export test-number?-true ()
            if (number? 42) 1 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-option-unwrap-or $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export test-option-unwrap-or ()
            option:unwrap-or (%none) 7
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-pow $ %{} 'CodeEntry (:doc "|pow via host import")
          :code $ quote $ defwasm-export test-pow (base exp) (pow base exp)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number 'Number
        'test-println $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export test-println () do (println 42) 1
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'test-range $ %{} 'CodeEntry (:doc "|range creates list of numbers")
          :code $ quote $ defwasm-export test-range ()
            &list:count $ range 5
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-range-sum $ %{} 'CodeEntry (:doc "|range 5 first+last: 0+4=4")
          :code $ quote $ defwasm-export test-range-sum ()
            &let
              xs $ range 5
              &+ (&list:nth xs 0) (&list:nth xs 4)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-range-two-args $ %{} 'CodeEntry (:doc "|range 2 5 creates 3 elements")
          :code $ quote $ defwasm-export test-range-two-args ()
            &list:count $ range 2 5
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-rem $ %{} 'CodeEntry (:doc |remainder)
          :code $ quote $ defwasm-export test-rem (a b) (&number:rem a b)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number 'Number
        'test-rest-count $ %{} 'CodeEntry (:doc "|rest args count: 3 extras")
          :code $ quote $ defwasm-export test-rest-count ()
            &list:count $ collect-rest 1 2 3 4
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-rest-empty $ %{} 'CodeEntry (:doc "|rest args with no extras: 10+20 = 30")
          :code $ quote $ defwasm-export test-rest-empty () (sum-rest 10 20)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-rest-sum $ %{} 'CodeEntry (:doc "|rest args: 1+2+3+4+5 = 15")
          :code $ quote $ defwasm-export test-rest-sum () (sum-rest 1 2 3 4 5)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-result-unwrap-or $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export test-result-unwrap-or ()
            result:unwrap-or (%err 3) 7
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-round $ %{} 'CodeEntry (:doc "|round function")
          :code $ quote $ defwasm-export test-round (x) (round x)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number
        'test-set-contains-method $ %{} 'CodeEntry (:doc "|.contains? dispatches on set")
          :code $ quote $ defwasm-export test-set-contains-method ()
            &+
              if
                contains? (#{} 10 20 30) 20
                , 1 0
              if
                contains? (#{} 10 20 30) 99
                , 10 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-set-count $ %{} 'CodeEntry (:doc "|set count")
          :code $ quote $ defwasm-export test-set-count ()
            &set:count $ #{} 10 20 30
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-set-difference $ %{} 'CodeEntry (:doc "|difference removes elements in second set")
          :code $ quote $ defwasm-export test-set-difference ()
            &set:count $ &difference (#{} 10 20 30 40) (#{} 20 40)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-set-difference-empty $ %{} 'CodeEntry (:doc "|difference with disjoint sets keeps all")
          :code $ quote $ defwasm-export test-set-difference-empty ()
            &set:count $ &difference (#{} 10 20) (#{} 30 40)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-set-empty $ %{} 'CodeEntry (:doc "|empty set")
          :code $ quote $ defwasm-export test-set-empty ()
            &+
              if
                &set:empty? $ #{}
                , 1 0
              if
                &set:empty? $ #{} 1
                , 10 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-set-empty-method $ %{} 'CodeEntry (:doc "|.empty returns an empty set")
          :code $ quote $ defwasm-export test-set-empty-method ()
            count $ empty $ #{} 10 20 30
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-set-exclude $ %{} 'CodeEntry (:doc "|exclude removes element")
          :code $ quote $ defwasm-export test-set-exclude ()
            &set:count $ &exclude (#{} 10 20 30) 20
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-set-include $ %{} 'CodeEntry (:doc "|include adds element")
          :code $ quote $ defwasm-export test-set-include ()
            &set:count $ &include (#{} 10 20) 30
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-set-includes $ %{} 'CodeEntry (:doc "|set includes value")
          :code $ quote $ defwasm-export test-set-includes ()
            &+
              if
                &set:includes? (#{} 10 20 30) 20
                , 1 0
              if
                &set:includes? (#{} 10 20 30) 99
                , 10 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-set-includes-method $ %{} 'CodeEntry (:doc "|.includes? dispatches on set")
          :code $ quote $ defwasm-export test-set-includes-method ()
            &+
              if
                includes? (#{} 10 20 30) 20
                , 1 0
              if
                includes? (#{} 10 20 30) 99
                , 10 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-set-max-method $ %{} 'CodeEntry (:doc "|.max dispatches on set")
          :code $ quote $ defwasm-export test-set-max-method ()
            option:unwrap-or
              max $ #{} 10 20 30 15
              , -1
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-set-min-method $ %{} 'CodeEntry (:doc "|.min dispatches on set")
          :code $ quote $ defwasm-export test-set-min-method ()
            option:unwrap-or
              min $ #{} 10 20 30 15
              , -1
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-set-union $ %{} 'CodeEntry (:doc "|union merges two sets")
          :code $ quote $ defwasm-export test-set-union ()
            &set:count $ &union (#{} 10 20) (#{} 20 30 40)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-set-union-same $ %{} 'CodeEntry (:doc "|union of identical sets")
          :code $ quote $ defwasm-export test-set-union-same ()
            &set:count $ &union (#{} 10 20 30) (#{} 10 20 30)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-sin $ %{} 'CodeEntry (:doc "|sin via host import")
          :code $ quote $ defwasm-export test-sin (x) (sin x)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number
        'test-sqrt $ %{} 'CodeEntry (:doc "|sqrt function")
          :code $ quote $ defwasm-export test-sqrt (x) (sqrt x)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number
        'test-static-option-result-inline-closures $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export test-static-option-result-inline-closures ()
            let
                offset 4
                option-value $ .unwrap-or
                  .map (%some 3)
                    fn (x)
                      hint-fn $ {}
                        :args $ [] 'Number
                        :return 'Number
                      + x offset
                  , 0
                result-value $ .unwrap-or
                  .map (%ok 5)
                    fn (x)
                      hint-fn $ {}
                        :args $ [] 'Number
                        :return 'Number
                      + x offset
                  , 0
                alias-value $ wasm-apply-via-alias 2 $ fn (x)
                  hint-fn $ {}
                    :args $ [] 'Number
                    :return 'Number
                  + x offset
                shadow-value $ wasm-apply-after-shadow 2 $ fn (x)
                  hint-fn $ {}
                    :args $ [] 'Number
                    :return 'Number
                  + x offset
              + option-value result-value alias-value shadow-value
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
          :tests $ [] $ %{} 'TestEntry
            :name |specializes-inline-callbacks-across-static-functions
            :code $ quote $ assert= 35 (test-static-option-result-inline-closures)
            :tags $ #{} :core :unit :wasm
        'test-static-option-result-methods $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export test-static-option-result-methods ()
            let
                option-value $ .unwrap-or
                  .map (%some 3) wasm-add-four
                  , 0
                result-value $ .unwrap-or
                  .map (%ok 5) wasm-add-four
                  , 0
              + option-value result-value
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |lowers-static-trait-methods)
            :code $ quote $ assert= 16 (test-static-option-result-methods)
            :tags $ #{} :core :unit :wasm
        'test-str-character-count $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export test-str-character-count () (&str:count "|A😀")
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-str-compare-eq $ %{} 'CodeEntry (:doc "|compare equal strings = 0")
          :code $ quote $ defwasm-export test-str-compare-eq () (&str:compare |abc |abc)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-str-compare-gt $ %{} 'CodeEntry (:doc "|compare abd > abc = 1")
          :code $ quote $ defwasm-export test-str-compare-gt () (&str:compare |abd |abc)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-str-compare-lt $ %{} 'CodeEntry (:doc "|compare abc < abd = -1")
          :code $ quote $ defwasm-export test-str-compare-lt () (&str:compare |abc |abd)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-str-concat $ %{} 'CodeEntry (:doc "|concat two strings and return character count")
          :code $ quote $ defwasm-export test-str-concat ()
            &str:count $ &str:concat |foo |bar
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-str-contains-false $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export test-str-contains-false () (&str:contains? |hello 10)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-str-contains-true $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export test-str-contains-true () (&str:contains? |hello 1)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-str-count $ %{} 'CodeEntry (:doc "|string character count")
          :code $ quote $ defwasm-export test-str-count () (&str:count |hello)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-str-empty-false $ %{} 'CodeEntry (:doc "|non-empty string has non-zero count")
          :code $ quote $ defwasm-export test-str-empty-false ()
            &= (&str:count |hi) 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-str-empty-true $ %{} 'CodeEntry (:doc "|rest of 1-char string has 0 characters")
          :code $ quote $ defwasm-export test-str-empty-true ()
            &=
              &str:count $ &str:rest |a
              , 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-str-escape $ %{} 'CodeEntry (:doc "|escape special chars")
          :code $ quote $ defwasm-export test-str-escape ()
            &str:count $ &str:escape |hello
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-str-find-index-found $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export test-str-find-index-found ()
            option:unwrap-or (str-find-index |hello |ell) -1
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-str-find-index-not-found $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export test-str-find-index-not-found ()
            option:unwrap-or (str-find-index |hello |xyz) -1
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-str-first $ %{} 'CodeEntry (:doc "|first byte of hello = 104 (h)")
          :code $ quote $ defwasm-export test-str-first () (&str:first |hello)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ []
        'test-str-includes-false $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export test-str-includes-false () (&str:includes? |hello |xyz)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-str-includes-true $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export test-str-includes-true () (&str:includes? |hello |ell)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-str-nth $ %{} 'CodeEntry (:doc "|nth character at index 1 of hello is e")
          :code $ quote $ defwasm-export test-str-nth ()
            if
              = (&str:nth |hello 1) |e
              , 1 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-str-pad-left $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export test-str-pad-left ()
            &str:count $ &str:pad-left |hi 5 |-
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-str-pad-right $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export test-str-pad-right ()
            &str:count $ &str:pad-right |hi 5 |-
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-str-rest $ %{} 'CodeEntry (:doc "|rest of hello has 4 characters")
          :code $ quote $ defwasm-export test-str-rest ()
            &str:count $ &str:rest |hello
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-str-slice $ %{} 'CodeEntry (:doc "|slice 1..4 from abcde = 3 characters (bcd)")
          :code $ quote $ defwasm-export test-str-slice ()
            &str:count $ &str:slice |abcde 1 4
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-str-utf8-byte-count $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export test-str-utf8-byte-count () (&str:utf8-byte-count "|A😀")
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-string-compare-method $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export test-string-compare-method () (&compare |abc |abd)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-struct-eq $ %{} 'CodeEntry (:doc "|struct definition equals source struct")
          :code $ quote $ defwasm-export test-struct-eq ()
            &let
              point $ %{} Point (:x 1) (:y 2)
              if
                &= (&struct:definition point) Point
                , 1 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-struct-field-tag $ %{} 'CodeEntry (:doc "|struct field-tag resolves by index")
          :code $ quote $ defwasm-export test-struct-field-tag ()
            &let
              point $ %{} Point (:x 1) (:y 2)
              if
                &= (&struct:field-tag point 0) :x
                , 1 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-struct-get-name $ %{} 'CodeEntry (:doc "|struct get-name returns struct tag")
          :code $ quote $ defwasm-export test-struct-get-name ()
            &let
              point $ %{} Point (:x 1) (:y 2)
              if
                &= (&struct:get-name point) :Point
                , 1 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-struct-matches-true $ %{} 'CodeEntry (:doc "|struct:matches? returns true for same type")
          :code $ quote $ defwasm-export test-struct-matches-true ()
            &let
              a $ %{} Point (:x 1) (:y 2)
              &let
                b $ %{} Point (:x 3) (:y 4)
                if (&struct:matches? a b) 1 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-struct-sum $ %{} 'CodeEntry (:doc "|Struct create + field access")
          :code $ quote $ defwasm-export test-struct-sum (x y)
            &let
              p $ %{} Point (:x x) (:y y)
              &+ (:x p) (:y p)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number 'Number
        'test-struct-to-map $ %{} 'CodeEntry (:doc "|struct to-map exposes field values by tag")
          :code $ quote $ defwasm-export test-struct-to-map ()
            &let
              point $ %{} Point (:x 1) (:y 2)
              &let
                m $ &struct:to-map point
                &+ (&map:get m :x) (&map:get m :y)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-tag-eq $ %{} 'CodeEntry (:doc "|Tag equality — same tags")
          :code $ quote $ defwasm-export test-tag-eq ()
            if (&= :ok :ok) 1 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-tag-neq $ %{} 'CodeEntry (:doc "|Tag inequality — different tags")
          :code $ quote $ defwasm-export test-tag-neq ()
            if (&= :ok :err) 1 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-to-pairs $ %{} 'CodeEntry (:doc "|to-pairs count")
          :code $ quote $ defwasm-export test-to-pairs ()
            &let
              ps $ to-pairs $ &{} :a 1 :b 2
              &+ (&list:count ps)
                &list:count $ &list:first ps
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-type-of-enum $ %{} 'CodeEntry (:doc "|type-of enum == :enum tag")
          :code $ quote $ defwasm-export test-type-of-enum ()
            if
              &=
                type-of $ :: :Pair 1 2
                , :enum
              , 1 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-type-of-list $ %{} 'CodeEntry (:doc "|type-of list == :list tag")
          :code $ quote $ defwasm-export test-type-of-list ()
            if
              &=
                type-of $ [] 1 2 3
                , :list
              , 1 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-type-of-map $ %{} 'CodeEntry (:doc "|type-of map == :map tag")
          :code $ quote $ defwasm-export test-type-of-map ()
            if
              &=
                type-of $ &{} :a 1
                , :map
              , 1 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-type-of-number $ %{} 'CodeEntry (:doc "|type-of number == :number tag")
          :code $ quote $ defwasm-export test-type-of-number ()
            if
              &= (type-of 42) :number
              , 1 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-type-of-set $ %{} 'CodeEntry (:doc "|type-of set == :set tag")
          :code $ quote $ defwasm-export test-type-of-set ()
            if
              &=
                type-of $ #{} 1 2
                , :set
              , 1 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'wasm-add-four $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export wasm-add-four (x)
            hint-fn $ {}
              :args $ [] 'Number
              :return 'Number
            + x 4
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number
        'wasm-apply-after-shadow $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export wasm-apply-after-shadow (value f)
            let
                before $ f value
                f 7
              + before f
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number $ :: 'Fn
              {} (:return 'Number)
                :args $ [] 'Number
        'wasm-apply-via-alias $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export wasm-apply-via-alias (value f)
            let
                g f
              g value
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number $ :: 'Fn
              {} (:return 'Number)
                :args $ [] 'Number
        'wasm-ffi-add $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export wasm-ffi-add (a b) (&+ a b)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number 'Number
        'wasm-ffi-async-echo $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export wasm-ffi-async-echo (text) text
          :examples $ []
          :schema $ :: 'Fn $ {} (:async true) (:return 'String)
            :args $ [] 'String
        'wasm-ffi-upcase $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export wasm-ffi-upcase (text) (host-string-upcase text)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ [] 'String
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns test-wasm.main
          :require $ test-wasm.helper :as helper
    'test-wasm.specialization-fail $ %{} 'FileEntry
      :defs $ {}
        'apply-one $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn apply-one (f) (f 1)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] $ :: 'Fn
              {} (:return 'Number)
                :args $ [] 'Number
        'escape-callback $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn escape-callback (f) f
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'Fn
              {} (:return 'Number)
                :args $ [] 'Number
            :return $ :: 'Fn $ {} (:return 'Number)
              :args $ [] 'Number
        'recursive-callback $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn recursive-callback (n f)
            if (> n 0)
              recursive-callback (- n 1) f
              f n
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number $ :: 'Fn
              {} (:return 'Number)
                :args $ [] 'Number
        'rest-callback $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn rest-callback (f & xs)
            f $ count xs
          :examples $ []
          :schema $ :: 'Fn $ {} (:rest 'Number) (:return 'Number)
            :args $ [] $ :: 'Fn
              {} (:return 'Number)
                :args $ [] 'Number
        'test-closure-escape $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn test-closure-escape ()
            escape-callback $ fn (x) (+ x 1)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-dynamic-closure-callee $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn test-dynamic-closure-callee ()
            let
                callee $ if true apply-one apply-one
              callee $ fn (x) (+ x 1)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-recursive-closure-specialization $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn test-recursive-closure-specialization ()
            recursive-callback 1 $ fn (x) (+ x 1)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-rest-closure-specialization $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn test-rest-closure-specialization ()
            rest-callback
              fn (x) (+ x 1)
              , 1 2
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-spread-closure-specialization $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn test-spread-closure-specialization ()
            rest-callback
              fn (x) (+ x 1)
              , & $ [] 1 2
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns test-wasm.specialization-fail

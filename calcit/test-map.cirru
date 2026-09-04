
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --full` first. Manual edits must follow format and schema conventions, then run `calcit edit format`.") (:package |test-map)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'test-map.main/main!) (:mode :native) (:reload-fn 'test-map.main/reload!)
      :feature-policy $ {}
      :modules $ [] |./util.cirru
      :type-slots $ {}
  :files $ {}
    'test-map.main $ %{} 'FileEntry
      :defs $ {}
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn main! () (log-title "|Testing maps") (test-maps) (log-title "|Testing map syntax") (test-native-map-syntax) (test-map-comma) (test-get) (test-shorthand) (test-filter-map-kv) (do true)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        'test-filter-map-kv $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn test-filter-map-kv ()
              assert=
                {} (:b 20) (:c 30)
                filter-map-kv
                  {} (:a 1) (:b 2) (:c 3)
                  fn (k v)
                    if (> v 1)
                      %:: MapEntryDecision :keep k $ * v 10
                      %:: MapEntryDecision :drop
              assert= ({})
                .filter-map-kv
                  {} $ :a 1
                  fn (k v) (%:: MapEntryDecision :drop)
          :examples $ []
          :schema $ :: 'Dynamic
        'test-get $ %{} 'CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing get")
              assert= (%none)
                get (&{}) :a
              assert= (%none)
                get-in (&{}) ([] :a :b)
              &let
                m $ &{} :a 1 :b 2 :c 3 :d 4
                assert=
                  first $ &map:destruct m
                  first $ &map:destruct m
                assert=
                  last $ &map:destruct m
                  last $ &map:destruct m
                assert= 3 $ count
                  option:unwrap $ last (&map:destruct m)
                assert= 10 $ foldl m 0
                  fn (acc pair)
                    let[] (k v) pair $ &+ acc v
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        'test-map-comma $ %{} 'CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing {,}")
              inside-eval: $ assert=
                macroexpand $ quote ({,} :a 1 :b 2 :c 3)
                quote $ pairs-map
                  section-by ([] :a 1 :b 2 :c 3) 2
              assert= ({,} :a 1 :b 2 :c 3)
                {} (:a 1) (:b 2) (:c 3)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        'test-maps $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn test-maps ()
              assert= 2 $ count
                {} (:a 1) (:b 2)
              let
                  dict $ merge
                    {} (:a 1) (:b 2)
                    {} (:c 3) (:d 5)
                assert= 4 $ count dict
                assert-detect identity $ contains? dict :a
                assert-detect not $ contains? dict :a2
                assert-detect identity $ includes? dict 2
                assert-detect not $ includes? dict 4
                ; println $ keys dict
                assert= (keys dict) (#{} :c :a :b :d)
                assert= (.keys dict) (#{} :c :a :b :d)
                assert=
                  vals $ {} (:a 1) (:b 2) (:c 2)
                  #{} 2 1
                assert= (assoc dict :h 10)
                  {} (:a 1) (:b 2) (:c 3) (:d 5) (:h 10)
                assert=
                  assoc (&{} :a 1 :b 2) :a 3
                  &{} :a 3 :b 2
                assert=
                  assoc (&{} :a 1 :b 2) :b 3
                  &{} :a 1 :b 3
                assert=
                  assoc (&{} :a 1 :b 2) :c 3
                  &{} :a 1 :b 2 :c 3
                assert=
                  assoc
                    assoc (&{} :a 1) :b 2
                    , :c 3
                  &{} :a 1 :b 2 :c 3
                inside-js: $ &let
                  data $ &{} :a 1
                  .!turnMap data
                  assert=
                    assoc (assoc data :b 2) :c 3
                    &{} :a 1 :b 2 :c 3
                assert= (dissoc dict :a) ({,} :b 2 :c 3 :d 5)
                assert= dict $ dissoc dict :h
                assert= (dissoc dict :a :b :c) (&{} :d 5)
                assert=
                  merge
                    {} (:a 1) (:b 2)
                    {} $ :c 3
                    {} $ :d 4
                  {} (:a 1) (:b 2) (:c 3) (:d 4)
                assert=
                  merge ({,} :a 1 :b 2 :c 3) ({,} :a nil :b 12) ({,} :c nil :d 14)
                  {,} :a nil :b 12 :c nil :d 14
                assert=
                  merge-non-nil ({,} :a 1 :b 2 :c 3) ({,} :a nil :b 12) ({,} :c nil :d 14)
                  {,} :a 1 :b 12 :c 3 :d 14
                assert=
                  merge
                    {} (:a true) (:b false) (:c true) (:d false)
                    {} (:a false) (:b false) (:c true) (:d true)
                  {} (:a false) (:b false) (:c true) (:d true)
                assert=
                  &hash $ &{} :a 1 :b 2 3 :c
                  &hash $ &{} 3 :c :a 1 :b 2
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        'test-native-map-syntax $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn test-native-map-syntax () $ inside-eval:
              assert=
                macroexpand $ quote
                  {} $ :a 1
                quote $ &{} :a 1
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        'test-shorthand $ %{} 'CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing shorthand")
              let
                  dict $ {} (:a 1)
                assert= 1 dict.:a
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote
          ns test-map.main $ :require
            [] util.core :refer $ [] log-title inside-eval: inside-js:

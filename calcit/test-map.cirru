
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |test-map) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'test-map.main/main!) (:mode :native) (:reload-fn 'test-map.main/reload!)
      :modules $ [] |./util.cirru
      :type-slots $ {}
  :files $ {}
    |test-map.main $ %{} 'FileEntry
      :defs $ {}
        |main! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn main! () (log-title "|Testing maps") (test-maps) (log-title "|Testing map syntax") (test-native-map-syntax) (test-map-comma) (test-get) (test-methods) (test-shorthand) (do true)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        |test-get $ %{} 'CodeEntry (:doc |)
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
        |test-map-comma $ %{} 'CodeEntry (:doc |)
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
        |test-maps $ %{} 'CodeEntry (:doc |)
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
                assert-detect not $ includes? dict :a
                ; println $ keys dict
                assert= (keys dict) (#{} :c :a :b :d)
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
                  merge
                    {} $ :a 1
                    , nil
                  {} $ :a 1
                assert=
                  &hash $ &{} :a 1 :b 2 3 :c
                  &hash $ &{} 3 :c :a 1 :b 2
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        |test-methods $ %{} 'CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing map methods")
              assert= (&{} :a 1 :b 2)
                .add (&{} :a 1) ([] :b 2)
              assert= (&{} :a 1 :b 2)
                .assoc (&{} :a 1) :b 2
              assert= true $ .contains? (&{} :a 1) :a
              assert= false $ .contains? (&{} :a 1) :b
              let
                  m $ {} (:a 1) (:b 2)
                inside-eval:
                  assert= m $ assert-traits m calcit.core/Len
                  assert= 2 $ .len m
                inside-js: $ assert= 2 (.count m)
              assert= (&{} :a 1)
                .dissoc (&{} :a 1 :b 2) :b
              assert= (&{} :a 1)
                .dissoc (&{} :a 1 :b 2 :c 3) :b :c
              assert= (&{})
                .empty $ &{} :a 1 :b 2
              assert= false $ .empty? (&{} :a 1 :b 2)
              assert= true $ .empty? (&{})
              assert= (%some 1)
                .get (&{} :a 1) :a
              assert= (%none)
                .get (&{} :a 1) :b
              assert= (%some 2)
                .get-in
                  {} $ :a
                    {} $ :b 2
                  [] :a :b
              assert= (%none)
                .get-in (&{}) ([] :a :b)
              assert= true $ .includes? (&{} :a 1 :b 2) 1
              assert= false $ .includes? (&{} :a 1 :b 2) 3
              assert= (#{} :a :b)
                .keys $ &{} :a 1 :b 2
              assert= (#{} :a :b)
                keys-non-nil $ &{} :a 1 :b 2 :c nil
              assert=
                {} (:a 11) (:b 12)
                .map (&{} :a 1 :b 2)
                  fn (entry)
                    [] (&list:first entry)
                      + 10 $ &list:last entry
              ; "not so stable, :bbbb is rare so it could be larger"
              let
                  mapped $ .map-list (&{} :a 1 :bbbb 2)
                    fn (entry)
                      [] (&list:first entry)
                        + 10 $ &list:last entry
                  _ $ assert-type mapped 'List
                assert=
                  [] ([] :a 11) ([] :bbbb 12)
                  .sort-by mapped &list:first
              assert=
                {} $ :a 11
                .map-kv
                  {} $ :a 1
                  fn (k v)
                    [] k $ + v 10
              assert=
                {} (:a 11) (:b 12)
                .map-kv
                  {} (:a 1) (:b 2) (:c 13)
                  fn (k v)
                    if (< v 10)
                      [] k $ + v 10
                      :: :none
              assert= (&{} :a 1 :b 2)
                .merge (&{} :a 1) (&{} :b 2)
              assert= (&{} :a 1 :b 2)
                select-keys (&{} :a 1 :b 2 :c 3) ([] :a :b)
              assert=
                [] $ [] :a 1
                .to-list $ {} (:a 1)
              let
                  pairs $ .to-list
                    {} (:a 1) (:b 2)
                inside-eval:
                  assert= pairs $ assert-traits pairs calcit.core/Len
                  assert= 2 $ .len pairs
                inside-js: $ assert= 2 (.count pairs)
              let
                  pairs $ .to-pairs
                    {} (:a 1) (:b 2)
                inside-eval:
                  assert= pairs $ assert-traits pairs calcit.core/Len
                  assert= 2 $ .len pairs
                inside-js: $ assert= 2 (.count pairs)
              assert= (&{} :a 1 :b 2)
                unselect-keys (&{} :a 1 :b 2 :c 3) ([] :c)
              assert= (#{} 1 2 3)
                .values $ &{} :a 1 :b 2 :c 3
              println $ .destruct (&{} :a 1 :b 2 :c 3)
              tag-match
                .destruct $ &{} :a 1 :b 2 :c 3
                (:none) (raise |expected-map-entry)
                (:some k v remaining)
                  do (assert-detect tag? k) (assert-detect number? v)
                    assert= 2 $ count remaining
              assert= (&{} :c 3)
                .diff-new (&{} :a 1 :b 2 :c 3) (&{} :a 2 :b 3)
              assert= (#{} :c)
                .diff-keys (&{} :a 1 :b 2 :c 3) (&{} :a 2 :b 3)
              assert= (#{} :a :b)
                .common-keys (&{} :a 1 :b 2 :c 3) (&{} :a 2 :b 3)
              let
                  triple $ .diff-triple (&{} :a 1 :b 2 :c 3) (&{} :a 2 :b 3)
                assert= (&list:nth triple 0) (#{} :c)
                assert= (&list:nth triple 1) (&{})
                assert=
                  count $ &list:nth triple 2
                  , 2
              assert= (&{} :a 1)
                .to-map $ &{} :a 1
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        |test-native-map-syntax $ %{} 'CodeEntry (:doc |)
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
        |test-shorthand $ %{} 'CodeEntry (:doc |)
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

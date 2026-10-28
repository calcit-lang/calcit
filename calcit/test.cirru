
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |app
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/reload!)
      :feature-policy $ {}
      :modules $ [] |./test-hygienic.cirru |./test-macro.cirru |./test-recursion.cirru |./test-edn.cirru |./test-js.cirru |./test-struct.cirru |./test-anonymous-enum.cirru |./test-algebra.cirru |./test-types.cirru |./test-types-inference.cirru |./test-generics.cirru |./test-enum.cirru |./test-traits.cirru |./test-doc-smoke.cirru |./test-def-meta.cirru |./util.cirru
      :type-slots $ {}
  :files $ {} $ 'app.main
    %{} 'FileEntry
      :defs $ {}
        '%A $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defimpl %A AtomDerefTrait
            .deref $ fn (self)
              match self $
                :atom x
                , x
          :examples $ []
          :schema $ quote Impl
        '%r $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defimpl %r DemoGetTrait
            .get $ fn (self) 1
          :examples $ []
          :schema $ quote Impl
        '*defref-demo $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defref *defref-demo 1
          :examples $ []
          :schema $ quote $ Ref Number
        '*ref-demo $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defref *ref-demo 0
          :examples $ []
          :schema $ quote $ Ref Number
        'AtomBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def AtomBox (impl-traits AtomBox0 %A)
          :examples $ []
          :schema $ quote EnumDef
        'AtomBox0 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum AtomBox (:atom 'Dynamic)
          :examples $ []
          :schema $ quote EnumDef
        'AtomDerefTrait $ %{} 'CodeEntry (:doc |)
          :code $ quote $ deftrait AtomDerefTrait (.deref 'Fn)
          :examples $ []
          :schema $ quote Trait
        'Demo $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def Demo (impl-traits Demo0 %r)
          :examples $ []
          :schema $ quote EnumDef
        'Demo0 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum Demo (:a 'Dynamic)
          :examples $ []
          :schema $ quote EnumDef
        'DemoGetTrait $ %{} 'CodeEntry (:doc |)
          :code $ quote $ deftrait DemoGetTrait (.get 'Fn)
          :examples $ []
          :schema $ quote Trait
        'Deref $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defimpl Deref DerefTrait
            .deref $ fn (self) 2
          :examples $ []
          :schema $ quote Impl
        'DerefTrait $ %{} 'CodeEntry (:doc |)
          :code $ quote $ deftrait DerefTrait (.deref 'Fn)
          :examples $ []
          :schema $ quote Trait
        'Num $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defimpl Num NumTrait
            .inc $ fn (x) (update x 1 inc)
            .show $ fn (x)
              str $ &enum:nth x 1
          :examples $ []
          :schema $ quote Impl
        'NumBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def NumBox (impl-traits NumBox0 Num)
          :examples $ []
          :schema $ quote EnumDef
        'NumBox0 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum NumBox (:number 'Number)
          :examples $ []
          :schema $ quote EnumDef
        'NumTrait $ %{} 'CodeEntry (:doc |)
          :code $ quote $ deftrait NumTrait (.inc 'Fn) (.show 'Fn)
          :examples $ []
          :schema $ quote Trait
        'ValueBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def ValueBox (impl-traits ValueBox0 Deref)
          :examples $ []
          :schema $ quote EnumDef
        'ValueBox0 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum ValueBox (:value 'Dynamic)
          :examples $ []
          :schema $ quote EnumDef
        'cross-module-counter-next $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn cross-module-counter-next (input)
            let
                typed $ assert-type input 'test-traits.external/Counter
              :value $ typed .next
          :examples $ []
          :schema $ quote $ Fn Dynamic (-> Number)
          :tests $ [] $ %{} 'TestEntry (:name |resolves-module-struct-trait)
            :code $ quote $ assert= 2
              cross-module-counter-next $ %{} test-traits.external/Counter $ :value 1
            :tags $ #{} :trait :unit
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () (&init-builtin-impls!)
            println $ &get-os
            println "|gen id:" $ generate-id!
            inside-js: $ load-console-formatter!
            log-title "|Testing tag function"
            test-tag
            util.core/log-title "|Testing detects"
            test-detects
            test-if
            test-display-stack
            test-cirru-parser
            test-fn
            test-macro/main!
            test-arguments
            test-try
            test-fn-eq
            test-refs
            test-method
            test-anonymous-enum
            test-effect
            inside-eval: $ test-hygienic/main!
            test-recursion/main!
            test-edn/main!
            test-json
            test-struct/main!
            test-anonymous-enum/main!
            inside-eval: $ test-algebra/main!
            test-types/main!
            test-types-inference/main!
            test-generics/main!
            test-enum/main!
            test-traits/main!
            assert= 2 $ cross-module-counter-next $ %{} test-traits.external/Counter (:value 1)
            test-doc-smoke/main!
            test-def-meta/main!
            test-buffer
            test-atom
            inside-js: $ test-js/main!
            do true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic) (:features :js-ffi)
        'query-search-node-kinds $ %{} 'CodeEntry
          :doc "|Fixture for machine-readable leaf and call search results"
          :code $ quote $ def query-search-node-kinds
            [] Option $ Option :none
          :examples $ []
          :schema $ quote $ List Dynamic
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'test-anonymous-enum $ %{} 'CodeEntry (:doc |)
          :code $ quote $ fn () (log-title "|Testing tuple")
            assert= :enum $ type-of $ :: :a :b
            assert= (Option :some :a)
              nth (:: :a :b) 0
            assert= (Option :some :b)
              nth (:: :a :b) 1
            assert= (Option :some :c)
              nth (:: :a :b :c) 2
            assert= 2 $ count $ :: :a :b
            assert= 3 $ count $ :: :a :b :c
            assert= 4 $ count $ :: :a :b :c :d
            assert= (Option :some :a)
              get (:: :a :b) 0
            assert= (Option :some :b)
              get (:: :a :b) 1
            assert= (Option :some :c)
              get (:: :a :b :c) 2
            assert= true $ contains? (:: :a :b :c) 2
            assert= (:: 1 0)
              update (:: 0 0) 0 inc
            assert= (:: 0 1)
              update (:: 0 0) 1 inc
            assert= (:: 1 0 0)
              update (:: 0 0 0) 0 inc
            assert= (:: 0 1 0)
              update (:: 0 0 0) 1 inc
            assert= (:: 0 0 1)
              update (:: 0 0 0) 2 inc
            assert= 1 $ count $ :: :none
            assert-detect enum? $ parse-cirru-edn "|:: :none"
            assert= false $ = (:: :t 1) (:: :t 2)
            assert= false $ = (:: :t 1) (:: :t 1 2)
            let
                a $ :: :a 1
                b $ %:: Demo :a 1
              assert= true $ any? (&enum:impls b)
                fn (impl)
                  includes? (str impl) |DemoGetTrait
              assert=
                &enum:params $ :: :a 1 2 3
                [] 1 2 3
              assert= "|(%:: 'Demo :a 1)" $ str b
            assert= "|(%:: _ :a :b :c)" $ str $ :: :a :b :c
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'test-arguments $ %{} 'CodeEntry (:doc |)
          :code $ quote $ fn () (log-title "|Testing arguments")
            ; "legacy-optional-contract: f1 pins `?` parameters hinted as Optional (E_LEGACY_OPTIONAL_PARAM era); f2 below is the Option form. Delete f1 when Optional is retired (0.32.0)."
            let
                f1 $ fn (a ? b c)
                  hint-fn $ Fn Tag (Optional Tag) (Optional Tag)
                    -> $ List $ Optional Tag
                  [] a b c
                f2 $ fn (a maybe-label maybe-count)
                  hint-fn $ Fn Number (Option String) (Option Number)
                    -> $ List Dynamic
                  [] a maybe-label maybe-count
              assert= (f1 :a) ([] :a nil nil)
              assert= (f1 :a :b) ([] :a :b nil)
              assert= (f1 :a :b :c) ([] :a :b :c)
              assert= (f2 1)
                [] 1 (Option :none) (Option :none)
              assert=
                f2 2 $ Option :some |ready
                [] 2 (Option :some |ready) (Option :none)
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'test-atom $ %{} 'CodeEntry (:doc |)
          :code $ quote $ fn ()
            let
                *a $ ref 1
              assert= 1 $ .deref *a
              assert= 1 $ &atom:deref *a
            assert= 1 $ .deref $ %:: AtomBox :atom 1
            assert= 1 $ .deref $ %:: AtomBox :atom 1
            assert= 2 $ .deref $ %:: AtomBox :atom 2
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'test-buffer $ %{} 'CodeEntry (:doc |)
          :code $ quote $ fn () (log-title |Buffer)
            println "|buffer value:" $ &buffer 0x11 |11
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'test-cirru-parser $ %{} 'CodeEntry (:doc |)
          :code $ quote $ fn () (log-title "|Testing Cirru parser")
            assert= (parse-cirru-list "|def f (a b) $ + a b")
              [] $ [] |def |f ([] |a |b) ([] |+ |a |b)
            assert= (parse-cirru-list "|{,} :a 1 :b false")
              [] $ [] |{,} |:a |1 |:b |false
            assert= (parse-cirru-list "|&call-dylib-edn 1")
              [] $ [] |&call-dylib-edn |1
            assert= (parse-cirru-list "|&call-dylib-edn-fn x")
              [] $ [] |&call-dylib-edn-fn |x
            assert= (parse-cirru-edn "|{} (:a 1) (:b ([] 3 |4 nil))")
              {} (:a 1)
                :b $ [] 3 |4 nil
            assert= "|[] |a |b $ [] |c |d" $ trim $ format-cirru-edn
              [] |a |b $ [] |c |d
            assert= "|a b $ c d" $ trim $ format-cirru
              [] $ [] |a |b $ [] |c |d
            assert=
              {} (:a 1)
                :b $ [] $ {} (:c 3) (4 5)
              tagging-edn $ {} (|a 1)
                :b $ [] $ {} (|c 3) (4 5)
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'test-detects $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn test-detects ()
            assert-detect fn? $ fn () 1
            assert-detect not $ bool? $ fn () 1
            assert-detect fn? &=
            inside-eval: $ assert-detect macro? cond
            assert-detect set? $ #{} 1 2 3
            assert= 1 $ either nil 1
            assert= 2 $ either 2 1
            assert= nil $ either nil nil
            assert= false $ either false true
            assert= false $ either nil false true
            assert= true $ either nil true
            assert= true $ either nil nil true
            assert= 2 $ either 2 $ raise "|should not be called"
            assert= 2 $ def x 2
            assert= false $ and true true false
            assert= false $ and true false true
            assert= true $ and true true true
            assert= false $ or false false false
            assert= true $ or false true false
            assert= true $ or false false true
            assert= true $ or false true
            assert= true $ or nil true
            assert=
              or true $ raise "|raise in or"
              , true
            assert=
              and false $ raise "|raise in and"
              , false
            assert= 2 $ when-not (> 1 2) 1 2
            assert= 1 $ if-not (> 2 1) 2 1
            assert= nil $ if-not (> 2 1) 2
            assert-detect identity $ /= 1 2
            assert-detect identity $ not= 1 2
            assert= true $ .ok? $ get-in ({,} :a 1) ([] :a)
            assert= false $ .ok? $ get-in ({,} :a 1) ([] :b)
            assert= false $ .ok? $ get-in nil ([] :b)
            assert= false $ .ok? $ get-in nil ([])
            assert= true $ .ok? $ get-in
              {,} :a $ [] 1
              [] :a 0
            assert= false $ .ok? $ get-in
              {,} :a $ [] 1
              [] :a 1
            assert= false $ .ok? $ get-in ([] 1 2 3) ([] :a)
            assert= 1 $ non-nil! 1
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'test-display-stack $ %{} 'CodeEntry (:doc |)
          :code $ quote $ fn () (log-title "|Testing display stack") (&display-stack "|show stack here")
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'test-effect $ %{} 'CodeEntry (:doc |)
          :code $ quote $ fn () (log-title "|Testing effect")
            println "|Env mode:" $ get-env |mode
            println "|Env mode:" $ option:unwrap-or (get-env |m0) "|default m0"
            eprintln "|stdout message"
            let
                missing-file $ fs:path |/calcit-result-contract-does-not-exist/file
                missing-dir $ fs:path |/calcit-result-contract-does-not-exist
              do
                assert= true $ result:err? missing-file.read-text
                assert= true $ result:err? missing-dir.read-dir
                assert= true $ result:err? missing-dir.walk-dir
                assert= true $ result:err? $ missing-file.write-text! |content
                assert-type missing-file.read-text $ quote $ Result String String
                assert-type missing-dir.read-dir $ quote $ Result (List FsPath) String
                assert-type missing-dir.walk-dir $ quote $ Result (List FsPath) String
                assert-type (missing-file.write-text! |content)
                  quote $ Result Unit String
          :examples $ []
          :schema $ quote $ Fn
            -> $ Result Unit String
        'test-fn $ %{} 'CodeEntry (:doc |)
          :code $ quote $ fn () (log-title "|Testing fn")
            &let
              empty-f $ fn $
              assert= &unit $ empty-f
            &let
              coll-f $ fn (& xs) xs
              assert= ([] 1 2 3 4 5)
                coll-f 1 & ([] 2 3 4) 5
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'test-fn-eq $ %{} 'CodeEntry (:doc |)
          :code $ quote $ fn () (log-title "|Testing equality of functions")
            let
                a $ fn (x) x
                b $ fn (x) x
              assert= a a
              assert= b b
              assert= false $ &= a b
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'test-if $ %{} 'CodeEntry (:doc |)
          :code $ quote $ fn () (log-title "|Testing if with nil")
            assert= (if false 1) (if nil 1)
            assert= (if false 1 2) (if nil 1 2)
            assert= (if false 1) (if &unit 1)
            assert= (if false 1 2) (if &unit 1 2)
            assert= true $ not &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'test-json $ %{} 'CodeEntry (:doc |)
          :code $ quote $ fn () (log-title "|Testing JSON")
            let
                parsed $ json-parse "|{\"name\":\"demo\",\"items\":[1,null,true],\"meta\":{\"flag\":false}}"
              assert= (Option :some |demo) (get parsed :name)
              assert=
                Option :some $ [] 1 nil true
                get parsed :items
              assert= (Result :ok false)
                get-in parsed $ [] :meta :flag
            assert=
              json-parse $ json-stringify $ {} (:status |ok)
                :items $ [] 1 true nil
              {} (:status |ok)
                :items $ [] 1 true nil
            assert= "|\"ok\"" $ json-stringify :ok
            assert= "|{\n  \"a\": 1\n}" $ json-pretty $ {} (:a 1)
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'test-method $ %{} 'CodeEntry (:doc |)
          :code $ quote $ fn () (log-title "|Testing method")
            let
                a0 $ %:: NumBox :number 0
                _ $ do (assert-type a0 NumBox)
                  let
                      a0-tuple a0
                    assert-type a0-tuple 'Tuple
                    assert= true $ any? (&enum:impls a0-tuple)
                      fn (impl)
                        = (impl-origin impl) (Option :some NumTrait)
              assert-traits a0 NumTrait calcit.core/Debug
              let
                  a1 $ .inc a0
                assert-type a1 NumBox
                assert-traits a1 NumTrait calcit.core/Debug
                let
                    a2 $ .inc a1
                  assert-traits a2 NumTrait calcit.core/Debug
                  assert= (%:: NumBox :number 2) a2
                  assert= |1 $ .show a1
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'test-refs $ %{} 'CodeEntry (:doc |)
          :code $ quote $ fn () (log-title "|Testing refs") (assert= 0 @*ref-demo)
            assert= &unit $ add-watch! *ref-demo :change $ fn (current prev) (println "|change happened:" prev current)
            assert= 2 $ reset! *ref-demo 2
            assert= &unit $ remove-watch! *ref-demo :change
            assert= "|remove-watch! failed: listener with key `missing` not found" $ try (remove-watch! *ref-demo :missing)
              fn (error) error
            assert= 2 @*ref-demo
            assert= :ref $ type-of *ref-demo
            let
                *l $ ref 1
              reset! *l 2
              assert= 2 @*l
            let
                *local $ ref 1
              assert= 1 @*defref-demo
              assert= 3 $ reset! *defref-demo 3
              assert= 3 @*defref-demo
              assert= :ref $ type-of *defref-demo
              swap! *local &+ 1
              assert= 2 @*local
              assert= (type-of *local) (type-of *ref-demo)
            let
                v $ %:: ValueBox :value 1
              assert-type v ValueBox
              assert= 2 $ .deref v
              assert= (Option :some 1) (nth v 1)
            let
                *b $ ref 0
                *c $ ref 0
              add-watch! *b :change $ fn (current prev)
                do
                  reset! *c $ assert-type current 'Number
                  , &unit
              reset! *b 1
              assert= 1 @*b
              assert= 1 @*c
            let
                watched $ ref 0
                notifications $ ref 0
              assert= &unit $ add-watch! watched :change $ fn (current prev) (assert= 1 current) (assert= 0 prev)
                reset! notifications $ inc @notifications
                , &unit
              assert= 1 $ reset! watched 1
              assert= 1 @notifications
              assert= &unit $ remove-watch! watched :change
              assert= 2 $ reset! watched 2
              assert= 1 @notifications
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'test-tag $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn test-tag ()
            ; assert "|tag function" $ =
              :a $ {} $ :a 1
              , 1
            ; inside-eval: $ &let
              base $ {} $ :a 1
              assert= 1 $ base :a
            inside-eval: $ assert= ([] 1)
              .map
                [] $ &{} :a 1
                , :a
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'test-try $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn test-try ()
            assert= false $ try
              do (println "|inside false try")
                raise "|error intented" $ [] :demo
                , :true
              fn (error)
                do (println "|Caught error:" error) false
            assert= |:a $ loop () $ try (raise |false)
              fn (error) (str :a)
            println "|Finished testing try"
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main
          :require (test-hygienic.main :as test-hygienic) (test-macro.main :as test-macro) (test-recursion.main :as test-recursion) (test-edn.main :as test-edn) (test-js.main :as test-js) (test-struct.main :as test-struct) (test-anonymous-enum.main :as test-anonymous-enum) (test-algebra.main :as test-algebra) (test-types.main :as test-types) (test-types-inference.main :as test-types-inference) (test-enum.main :as test-enum) (test-generics.main :as test-generics) (test-traits.main :as test-traits) (test-doc-smoke.main :as test-doc-smoke) (test-def-meta.main :as test-def-meta)
            util.core :refer $ log-title inside-eval: inside-js:

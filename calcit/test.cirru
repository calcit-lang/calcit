
{} (:package |app)
  :configs $ {} (:init-fn |app.main/main!) (:reload-fn |app.main/reload!)
    :modules $ [] |./test-cond.cirru |./test-hygienic.cirru |./test-lens.cirru |./test-list.cirru |./test-macro.cirru |./test-map.cirru |./test-math.cirru |./test-recursion.cirru |./test-set.cirru |./test-string.cirru |./test-edn.cirru |./test-js.cirru |./test-record.cirru |./test-nil.cirru |./test-fn.cirru |./test-tuple.cirru |./test-algebra.cirru |./test-types.cirru |./test-types-inference.cirru |./test-generics.cirru |./test-enum.cirru |./test-traits.cirru |./util.cirru
  :files $ {}
    |app.main $ %{} :FileEntry
      :defs $ {}
        |%Num $ %{} :CodeEntry (:doc |)
          :code $ quote (defrecord %Num :inc :show)
        |*ref-demo $ %{} :CodeEntry (:doc |)
          :code $ quote (defatom *ref-demo 0)
        |Num $ %{} :CodeEntry (:doc |)
          :code $ quote
            def Num $ %{} %Num
              :inc $ fn (x) (update x 1 inc)
              :show $ fn (x)
                str $ &tuple:nth x 1
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! ()
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
              test-tuple
              test-effect
              inside-eval: $ test-hygienic/main!
              test-cond/main!
              test-lens/main!
              test-list/main!
              test-map/main!
              test-math/main!
              test-recursion/main!
              test-set/main!
              test-string/main!
              test-edn/main!
              test-record/main!
              test-nil/main!
              test-fn/main!
              test-tuple/main!
              test-algebra/main!
              test-types/main!
              test-types-inference/main!
              test-generics/main!
              test-enum/main!
              test-traits/main!
              test-buffer
              test-atom
              inside-js: $ test-js/main!
              do true
        |reload! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn reload! () $ :: :unit
        |test-atom $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn ()
              let
                  *a $ atom 1
                assert= 1 $ deref *a
                assert= 1 $ &atom:deref *a
              let
                  %A $ defrecord! %A
                    :deref $ fn (self)
                      tag-match self
                        (:atom x) x
                assert= 1 $ deref $ &tuple:with-impls (:: :atom 1) %A
                assert= 1 $ deref $ &tuple:with-impls (:: :atom 1) %A
                assert= 2 $ deref $ &tuple:with-impls (:: :atom 2) %A
        |test-arguments $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing arguments")
              let
                  f1 $ fn (a ? b c)
                    assert-type a :tag
                    assert-type b $ :: :optional :tag
                    assert-type c $ :: :optional :tag
                    [] a b c
                assert= (f1 :a) ([] :a nil nil)
                assert= (f1 :a :b) ([] :a :b nil)
                assert= (f1 :a :b :c) ([] :a :b :c)
        |test-buffer $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () (log-title |Buffer)
              println "|buffer value:" $ &buffer 0x11 |11
        |test-cirru-parser $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing Cirru parser")
              assert= (parse-cirru-list "|def f (a b) $ + a b")
                [] $ [] |def |f ([] |a |b) ([] |+ |a |b)
              assert= (parse-cirru-list "|{,} :a 1 :b false")
                [] $ [] |{,} |:a |1 |:b |false
              assert= (parse-cirru-edn "|{} (:a 1) (:b ([] 3 |4 nil))")
                {} (:a 1)
                  :b $ [] 3 |4 nil
              assert= "|[] |a |b $ [] |c |d" $ trim
                format-cirru-edn $ [] |a |b ([] |c |d)
              assert= "|a b $ c d" $ trim
                format-cirru $ []
                  [] |a |b $ [] |c |d
              assert=
                {} (:a 1)
                  :b $ []
                    {} (:c 3) (4 5)
                tagging-edn $ {} (|a 1)
                  :b $ []
                    {} (|c 3) (4 5)
        |test-detects $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-detects ()
              assert-detect fn? $ fn () 1
              assert-detect not $ bool?
                fn () 1
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
              assert= 2 $ either 2 (raise "|should not be called")
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
              assert= true $ some-in? ({,} :a 1) ([] :a)
              assert= false $ some-in? ({,} :a 1) ([] :b)
              assert= false $ some-in? nil ([] :b)
              assert= false $ some-in? nil ([])
              assert= true $ some-in?
                {,} :a $ [] 1
                [] :a 0
              assert= false $ some-in?
                {,} :a $ [] 1
                [] :a 1
              assert= false $ some-in? ([] 1 2 3) ([] :a)
              assert= 1
                non-nil! 1
        |test-display-stack $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing display stack") (&display-stack "|show stack here")
        |test-effect $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing effect")
              println "|Env mode:" $ get-env |mode
              println "|Env mode:" $ get-env |m0 "|default m0"
              eprintln "|stdout message"
        |test-fn $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing fn")
              &let
                empty-f $ fn ()
                assert= nil $ empty-f
              &let
                coll-f $ fn (& xs) xs
                assert= ([] 1 2 3 4 5)
                  coll-f 1 & ([] 2 3 4) 5
        |test-fn-eq $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing equality of functions")
              let
                  a $ fn (x) x
                  b $ fn (x) x
                assert= a a
                assert= b b
                assert= false $ &= a b
        |test-if $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing if with nil")
              assert= (if false 1) (if nil 1)
              assert= (if false 1 2) (if nil 1 2)
        |test-method $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing method")
              let
                  a $ &tuple:with-impls (:: :calcit/number 0) Num
                assert= (&tuple:with-impls (:: :calcit/number 2) Num) (-> a .inc .inc)
                assert= |1 $ -> a .inc .show
                assert-detect record? $ &list:first $ &tuple:impls a
        |test-refs $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing refs") (assert= 0 @*ref-demo)
              add-watch *ref-demo :change $ fn (current prev) (println "|change happened:" prev current)
              reset! *ref-demo 2
              remove-watch *ref-demo :change
              assert= 2 @*ref-demo
              assert= :ref $ type-of *ref-demo
              let
                  *l $ atom 1
                reset! *l 2
                assert= 2 @*l

              let
                  Deref $ defrecord! Deref
                    :deref $ fn (self) 2
                  v $ &tuple:with-impls (:: :value 1) Deref
                assert= 2 @v
                assert= (nth v 1) 1

              let
                  *b $ atom 0
                  *c $ atom 0
                add-watch *b :change $ fn (current prev)
                  reset! *c current
                reset! *b 1
                assert= 1 @*b
                assert= 1 @*c
        |test-tag $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-tag ()
              ; assert "|tag function" $ =
                :a $ {} (:a 1)
                , 1
              ; inside-eval: $ &let
                base $ {} (:a 1)
                assert= 1 $ base :a
              inside-eval: $ assert= ([] 1)
                .map
                  [] $ &{} :a 1
                  , :a
        |test-try $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing try")
              assert= :true $ try
                do (println "|inside try") :true
                fn $ error
              assert= :false $ try
                do (println "|inside false try")
                  raise "|error intented" $ [] :demo
                  , :true
                fn (error)
                  do (println "|Caught error:" error) :false
              assert= |:a $ apply-args ()
                fn () $ try (raise |false)
                  fn (error) (str :a)
              println "|Finished testing try"
        |test-tuple $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing tuple")
              assert= :tuple $ type-of (:: :a :b)
              assert= :a $ nth (:: :a :b) 0
              assert= :b $ nth (:: :a :b) 1
              assert= :c $ nth (:: :a :b :c) 2
              assert= 2 $ count (:: :a :b)
              assert= 3 $ count (:: :a :b :c)
              assert= 4 $ count (:: :a :b :c :d)
              assert= :a $ get (:: :a :b) 0
              assert= :b $ get (:: :a :b) 1
              assert= :c $ get (:: :a :b :c) 2
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
              assert= 1 $ count (:: :none)
              assert-detect tuple? $ parse-cirru-edn "|:: :none"
              assert= false $ = (:: :t 1) (:: :t 2)
              assert= false $ = (:: :t 1) (:: :t 1 2)
              let
                  a $ :: :a 1
                  %r $ defrecord! %demo
                    :get $ fn (self) 1
                  b $ &tuple:with-impls a %r
                assert= %r $ &list:first $ &tuple:impls b
                assert=
                  &tuple:params $ :: :a 1 2 3
                  [] 1 2 3
                assert= "|(:: :a 1 (:impls %demo))" $ str b
              assert= "|(:: :a :b :c)" $ str (:: :a :b :c)
      :ns $ %{} :CodeEntry (:doc |)
        :code $ quote
          ns app.main $ :require (test-cond.main :as test-cond) (test-hygienic.main :as test-hygienic) (test-lens.main :as test-lens) (test-list.main :as test-list) (test-macro.main :as test-macro) (test-map.main :as test-map) (test-math.main :as test-math) (test-recursion.main :as test-recursion) (test-set.main :as test-set) (test-string.main :as test-string) (test-edn.main :as test-edn) (test-js.main :as test-js) (test-record.main :as test-record) (test-nil.main :as test-nil) (test-fn.main :as test-fn) (test-tuple.main :as test-tuple) (test-algebra.main :as test-algebra) (test-types.main :as test-types) (test-types-inference.main :as test-types-inference) (test-enum.main :as test-enum) (test-generics.main :as test-generics) (test-traits.main :as test-traits)
            util.core :refer $ log-title inside-eval: inside-js:

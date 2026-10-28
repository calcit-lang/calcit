
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |app
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/main!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'app.main
    %{} 'FileEntry
      :defs $ {}
        'P $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct P (:age 'Number) (:name 'String)
          :examples $ []
          :schema $ quote StructDef
        'case-arg-defn $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-arg-defn ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> Number)
                  take-num $ if flag |bad
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-arg-defn-nested $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-arg-defn-nested ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> Number)
                  take-num $ if flag (if flag |bad) 1
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-arg-defn-niljoin $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-arg-defn-niljoin ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> Number)
                  take-num $ if flag nil |bad
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-arg-defn-niljoin-ok $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-arg-defn-niljoin-ok ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> Number)
                  take-num $ if flag 1 nil
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-arg-defn-ok $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-arg-defn-ok ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> Number)
                  take-num $ if flag 1
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-arg-defn-when $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-arg-defn-when ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> Number)
                  take-num $ when flag |bad
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-arg-local $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-arg-local ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> Unit)
                  let
                      take $ fn (v)
                        hint-fn $ quote $ Fn Number (-> Unit)
                        , &unit
                    take $ if flag |bad
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-arg-local-ok $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-arg-local-ok ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> Unit)
                  let
                      take $ fn (v)
                        hint-fn $ quote $ Fn Number (-> Unit)
                        , &unit
                    take $ if flag 1
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-arg-method-count $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-arg-method-count ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> Number)
                  .count $ if flag 1
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-arg-method-ok $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-arg-method-ok ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> String)
                  .slice |abcd (if flag 1) 2
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-arg-method-slice $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-arg-method-slice ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> String)
                  .slice |abcd (if flag |bad) 2
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-arg-method-slice-recv $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-arg-method-slice-recv ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> String)
                  .slice (if flag 1) 0 2
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-arg-proc-add $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-arg-proc-add ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> Number)
                  &+ 1 $ if flag |bad
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-arg-proc-plus $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-arg-proc-plus ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> Number)
                  + 1 $ if flag |bad
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-arg-proc-strcount $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-arg-proc-strcount ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> Number)
                  &str:count $ if flag 1
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-assert-type $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-assert-type ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> Unit)
                  do
                    assert-type (if flag |bad) 'Number
                    , &unit
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-direct-cross-def $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-direct-cross-def ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> String)
                  take-str $ g flag
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-hof-ok $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-hof-ok ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> Unit)
                  let
                      use $ fn (f)
                        hint-fn $ quote $ Fn
                          Fn Number $ -> Number
                          -> Unit
                        , &unit
                    use $ fn (x) (if flag x)
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-hof-typed-fn-arg $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-hof-typed-fn-arg ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> Unit)
                  let
                      use $ fn (f)
                        hint-fn $ quote $ Fn
                          Fn Number $ -> Number
                          -> Unit
                        , &unit
                    use $ fn (x) (if flag |bad)
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-let-bound-arg-defn $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-let-bound-arg-defn ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> Number)
                  let
                      y $ if flag |bad
                    take-num y
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-let-bound-fn-literal $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-let-bound-fn-literal ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> String)
                  let
                      f $ fn () $ if flag 1
                    take-str $ f
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-let-bound-list $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-let-bound-list ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> String)
                  let
                      xs $ [] $ if flag 1
                    take-str $ nth xs 0
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-let-bound-nilq $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-let-bound-nilq ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> Bool)
                  let
                      y $ if flag 1
                    nil? y
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-let-bound-ok-arg $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-let-bound-ok-arg ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> Number)
                  let
                      y $ if flag 1
                    take-num y
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-let-bound-plus $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-let-bound-plus ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> Number)
                  let
                      y $ if flag 1
                    + y 1
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-let-bound-ret $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-let-bound-ret ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> Number)
                  let
                      y $ if flag |bad
                    , y
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-let-cross-def $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-let-cross-def ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> String)
                  let
                      y $ g flag
                    take-str y
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-let-cross-def2 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-let-cross-def2 ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> String)
                  let
                      y $ g2 flag
                    take-str y
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-list-literal $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-list-literal ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool
                    -> $ List Number
                  [] 1 $ if flag |bad
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-list-ret-ok $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-list-ret-ok ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool
                    -> $ List Number
                  [] 1 $ if flag 2
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-local-ok $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-local-ok ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> Unit)
                  let
                      eat $ fn (v)
                        hint-fn $ quote $ Fn Number (-> Unit)
                        , &unit
                    eat $ if flag 1
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-local-ret-typed-fn-literal $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-local-ret-typed-fn-literal ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> Unit)
                  let
                      h $ fn (y)
                        hint-fn $ quote $ Fn Bool (-> Number)
                        if y 1
                    do (h flag) &unit
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-m-slice-bad $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-m-slice-bad ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> String)
                  .slice |abcd $ if flag |x
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-m-slice-ok $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-m-slice-ok ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> String)
                  .slice |abcd $ if flag 1
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-m-slice-ok2 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-m-slice-ok2 ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> String)
                  .slice |abcd (if flag 1) (Option :some 2)
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-map-callback $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-map-callback ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool
                    -> $ List Number
                  map ([] 1 2)
                    fn (x) (if flag |bad)
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-map-callback-ok $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-map-callback-ok ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool
                    -> $ List Number
                  map ([] 1 2)
                    fn (x) (if flag x)
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-n3-dynamic-ret $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-n3-dynamic-ret ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> Dynamic)
                  if flag 1
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-n3-jsnullish-arg $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-n3-jsnullish-arg ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> Unit)
                  take-js $ if flag 1
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-n3-jsnullish-ret $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-n3-jsnullish-ret ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool
                    -> $ JsNullish Number
                  if flag 1
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-n3-jsnullish-ret-join $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-n3-jsnullish-ret-join ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool
                    -> $ JsNullish Number
                  if flag 1 nil
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-n3-let-value $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-n3-let-value ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> Dynamic)
                  let
                      y $ if flag 1
                    , y
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-n3-nil-option $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-n3-nil-option ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool
                    -> $ Option Number
                  nil->option $ if flag 1
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-n3-statement $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-n3-statement ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> Number)
                  do
                    if flag $ take-num 1
                    , 1
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-qfield-js-nullish-option $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-qfield-js-nullish-option ()
            let
                p $ P :age 1 :name |a
              js-nullish->option $ .?-name p
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic) (:features :js-ffi)
        'case-qfield-js-present $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-qfield-js-present ()
            let
                p $ P :age 1 :name |a
              js-present? $ .?-name p
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic) (:features :js-ffi)
        'case-qfield-let-take-str $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-qfield-let-take-str ()
            let
                p $ P :age 1 :name |a
                n $ .?-name p
              take-str n
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic) (:features :js-ffi)
        'case-qfield-nil-option $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-qfield-nil-option ()
            let
                p $ P :age 1 :name |a
              nil->option $ .?-name p
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic) (:features :js-ffi)
        'case-qfield-nilq $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-qfield-nilq ()
            let
                p $ P :age 1 :name |a
              nil? $ .?-name p
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic) (:features :js-ffi)
        'case-qfield-ret-opt $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-qfield-ret-opt ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool
                    -> $ Optional String
                  .?-name $ P :age 1 :name |a
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic) (:features :js-ffi)
        'case-qfield-str-concat $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-qfield-str-concat ()
            let
                p $ P :age 1 :name |a
              str $ .?-name p
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic) (:features :js-ffi)
        'case-qfield-take-str $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-qfield-take-str ()
            let
                p $ P :age 1 :name |a
              take-str $ .?-name p
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic) (:features :js-ffi)
        'case-ret-bool-ok $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-ret-bool-ok ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> Bool)
                  if flag true
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-ret-case $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-ret-case ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> Number)
                  case flag $ true |bad
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-ret-cond $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-ret-cond ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> Number)
                  cond $ flag |bad
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-ret-cond2 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-ret-cond2 ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> Number)
                  cond (flag |bad)
                    (not flag) 2
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-ret-do-body $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-ret-do-body ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> Number)
                  do (println 1) (if flag |bad)
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-ret-if-niljoin $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-ret-if-niljoin ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> Number)
                  if flag |bad nil
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-ret-if-noelse $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-ret-if-noelse ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> Number)
                  if flag |bad
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-ret-let-body $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-ret-let-body ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> Number)
                  let
                      y 1
                    if flag |bad
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-ret-nested-if $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-ret-nested-if ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> Number)
                  if flag (if flag |bad) 1
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-ret-nested-if-else-nil $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-ret-nested-if-else-nil ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> Number)
                  if flag 1 $ if flag |bad
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-ret-ok-match $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-ret-ok-match ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> Number)
                  if flag 1
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-ret-option $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-ret-option ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool
                    -> $ Option Number
                  if flag 1
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-ret-option-bad $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-ret-option-bad ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool
                    -> $ Option Number
                  if flag |bad
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-ret-string-niljoin-ok $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-ret-string-niljoin-ok ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> String)
                  if flag |a nil
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-ret-when $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-ret-when ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> Number)
                  when flag |bad
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-struct-ctor-bad $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-struct-ctor-bad ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> Dynamic)
                  P :age (if flag |bad) :name |a
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-struct-ctor-nil $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-struct-ctor-nil ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> Dynamic)
                  P :age (if flag 1) :name |a
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-struct-field $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-struct-field ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> Dynamic)
                  P
                    :age $ if flag |bad
                    :name |a
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-struct-field-pct $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-struct-field-pct ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> Dynamic)
                  %{} P
                    :age $ if flag |bad
                    :name |a
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'case-struct-pct-nil $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn case-struct-pct-nil ()
            let
                probe $ fn (flag)
                  hint-fn $ quote $ Fn Bool (-> Dynamic)
                  %{} P
                    :age $ if flag 1
                    :name |a
              probe true
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'g $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn g (flag) (if flag 1)
          :examples $ []
          :schema $ quote $ Fn Bool (-> Dynamic)
        'g2 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn g2 (flag) (if flag 1 nil)
          :examples $ []
          :schema $ quote $ Fn Bool (-> Dynamic)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'take-js $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn take-js (n) &unit
          :examples $ []
          :schema $ quote $ Fn (JsNullish Number) (-> Unit)
        'take-num $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn take-num (n) n
          :examples $ []
          :schema $ quote $ Fn Number (-> Number)
        'take-str $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn take-str (s) s
          :examples $ []
          :schema $ quote $ Fn String (-> String)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main (:require)

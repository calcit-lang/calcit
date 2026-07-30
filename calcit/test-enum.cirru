
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |test-enum) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:description |) (:init-fn |test-enum.main/main!) (:mode :native) (:reload-fn |test-enum.main/reload!)
      :modules $ []
      :type-slots $ {}
  :files $ {}
    |test-enum.main $ %{} :FileEntry
      :defs $ {}
        |Duo $ %{} :CodeEntry (:doc "|Generic enum with 2 type variables") (:schema :dynamic)
          :code $ quote
            defenum Duo ('T 'U) (:pair 'T 'U) (:swapped 'U 'T)
          :examples $ []
        |Maybe1 $ %{} :CodeEntry (:doc "|Generic enum with 1 type variable") (:schema :dynamic)
          :code $ quote
            defenum Maybe1 ('T) (:some 'T) (:none)
          :examples $ []
        |Result0 $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defenum Result0 (:err :string) (:ok)
          :examples $ []
        |ResultImpl $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defimpl ResultImpl ResultTrait $ .dummy nil
          :examples $ []
        |ResultTrait $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            deftrait ResultTrait $ .dummy :fn
          :examples $ []
        |ShownBox $ %{} :CodeEntry (:doc "|Generic struct with where-bound on payload type") (:schema :dynamic)
          :code $ quote
            defstruct ShownBox ('T)
              ({} ('T Show))
              (:value 'T)
          :examples $ []
        |ShownMaybe $ %{} :CodeEntry (:doc "|Generic enum with where-bound on payload type") (:schema :dynamic)
          :code $ quote
            defenum ShownMaybe ('T)
              ({} ('T Show))
              (:some 'T)
              (:none)
          :examples $ []
        |check-result-type $ %{} :CodeEntry (:doc "|Check if value has enum origin")
          :code $ quote
            defn check-result-type (r)
              some? $ &tuple:enum r
          :examples $ []
          :schema $ :: :fn
            {} (:return :bool)
              :args $ [] 'test-enum.main/Result0
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! () $ do (println "|Testing enum runtime validation...") (test-enum-creation) (test-generic-enum-creation) (test-generic-enum-where-bounds) (test-where-bound-definitions) (test-tag-match-validation) (test-tuple-to-enum) (test-match) (println "|All tests passed!")
          :examples $ []
          :schema $ :: :fn
            {} (:return :unit)
              :args $ []
        |reload! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn reload! () $ println |Reloaded
          :examples $ []
          :schema $ :: :fn
            {} (:return :unit)
              :args $ []
        |takes-result $ %{} :CodeEntry (:doc "|Function accepting Result0 enum type")
          :code $ quote
            defn takes-result (r)
              tag-match r
                (:ok) :ok
                (:err msg) msg
                _ :unknown
          :examples $ []
          :schema $ :: :fn
            {} (:return :dynamic)
              :args $ [] 'test-enum.main/Result0
        |test-enum-creation $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-enum-creation () $ do (println "|Testing enum tuple creation...")
              let
                  valid-ok $ %:: Result0 :ok
                  Result1 $ impl-traits Result0 ResultImpl
                assert= :ok $ &tuple:nth valid-ok 0
                let
                    ok-impl $ %:: Result1 :ok
                  assert= true $ any? (&tuple:impls ok-impl)
                    fn (impl)
                      includes? (str impl) |ResultTrait
                  assert= "|(%:: :ok (:enum Result0))" $ str ok-impl
              let
                  valid-err $ %:: Result0 :err |error-msg
                assert= :err $ &tuple:nth valid-err 0
                assert= true $ tuple? valid-err
              ; Test invalid tag $ should fail - uncomment to see error
              ; let
                (invalid (%:: Result0 :invalid))
                raise "|Should have failed with invalid tag"
              ; Test wrong arity $ should fail - uncomment to see error
              ; let
                (wrong-arity (%:: Result0 :ok |extra))
                raise "|Should have failed with wrong arity"
              println "|✓ Enum creation validation passed"
          :examples $ []
          :schema $ :: :fn
            {} (:return :unit)
              :args $ []
        |test-generic-enum-creation $ %{} :CodeEntry (:doc "|Exercise defenum generic variables in runtime creation and matching")
          :code $ quote
            defn test-generic-enum-creation () $ do (println "|Testing generic enum creation...")
              let
                  some-num $ %:: Maybe1 :some 1
                  none-value $ %:: Maybe1 :none
                  pair-value $ %:: Duo :pair 1 |hi
                  swapped-value $ %:: Duo :swapped |hi 1
                assert-type some-num $ :: 'Maybe1 :number
                assert-type none-value $ :: 'Maybe1 :dynamic
                assert-type pair-value $ :: 'Duo :number :string
                assert-type swapped-value $ :: 'Duo :number :string
                assert= 1 $ unwrap-maybe (%:: Maybe1 :some 1)
                assert= nil $ unwrap-maybe (%:: Maybe1 :none)
                assert= :pair $ &tuple:nth pair-value 0
                assert= 1 $ &tuple:nth pair-value 1
                assert= |hi $ &tuple:nth pair-value 2
              println "|✓ Generic enum creation passed"
          :examples $ []
          :schema $ :: :fn
            {} (:return :unit)
              :args $ []
        |test-generic-enum-where-bounds $ %{} :CodeEntry (:doc "|Exercise where-bounds with generic enum payloads")
          :code $ quote
            defn test-generic-enum-where-bounds () $ let
                render-maybe $ fn (v)
                  hint-fn $ {}
                    :generics $ [] 'T
                    :where $ {} ('T Show)
                    :args $ [] (:: 'Maybe1 'T)
                    :return :string
                  match v
                    (:none) |none
                    (:some item) (.show item)
                render-duo $ fn (v)
                  hint-fn $ {}
                    :generics $ [] 'T 'U
                    :where $ {} ('T Show) ('U Show)
                    :args $ [] (:: 'Duo 'T 'U)
                    :return :string
                  match v
                    (:pair left right)
                      str-spaced |pair (.show left) (.show right)
                    (:swapped right left)
                      str-spaced |swapped (.show right) (.show left)
              println "|Testing generic enum where-bounds..."
              assert= |1 $ render-maybe (%:: Maybe1 :some 1)
              assert= |none $ render-maybe (%:: Maybe1 :none)
              assert= (str-spaced |pair |1 |hi)
                render-duo $ %:: Duo :pair 1 |hi
              assert= (str-spaced |swapped |hi |1)
                render-duo $ %:: Duo :swapped |hi 1
              println "|✓ Generic enum where-bounds passed"
          :examples $ []
          :schema $ :: :fn
            {} (:return :unit)
              :args $ []
        |test-match $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defn test-match ()
              let
                  result-ok $ %:: Result0 :ok
                  v $ match result-ok
                    (:ok) :matched-ok
                    (:err msg) msg
                assert= :matched-ok v
              let
                  result-err $ %:: Result0 :err |some-error
                  v $ match result-err
                    (:ok) :matched-ok
                    (:err msg) msg
                assert= |some-error v
              ; Test exhaustive match with wildcard
              let
                  result-ok $ %:: Result0 :ok
                  v $ match result-ok
                    (:ok) :ok-branch
                    _ :default-branch
                assert= :ok-branch v
              println "|✓ match syntax passed"
          :examples $ []
        |test-tag-match-validation $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-tag-match-validation () $ do (println "|Testing tag-match runtime validation...")
              let
                  result $ %:: Result0 :ok
                  v $ tag-match result
                    (:ok) :ok
                    _ :unknown
                assert= :ok v
              println "|✓ Tag-match validation passed"
          :examples $ []
          :schema $ :: :fn
            {} (:return :unit)
              :args $ []
        |test-tuple-to-enum $ %{} :CodeEntry (:doc "|Test automatic tuple-to-enum rewrite")
          :code $ quote
            defn test-tuple-to-enum () $ do (println "|Testing tuple-to-enum rewrite...") (; Untyped tuple :: :ok gets rewritten to %:: Result0 :ok)
              assert= :ok $ takes-result (:: :ok)
              ; Untyped tuple with payload
              assert= |error-msg $ takes-result (:: :err |error-msg)
              ; Verify the rewritten value has enum origin
              assert= true $ check-result-type (:: :ok)
              println "|✓ Tuple-to-enum rewrite passed"
          :examples $ []
          :schema $ :: :fn
            {} (:return :unit)
              :args $ []
        |test-where-bound-definitions $ %{} :CodeEntry (:doc "|Exercise defstruct/defenum where-map syntax on generic data types")
          :code $ quote
            defn test-where-bound-definitions () $ do (println "|Testing data definition where-bounds...")
              let
                  box $ %{} ShownBox (:value 1)
                let
                    some-value $ %:: ShownMaybe :some 1
                  let
                      none-value $ %:: ShownMaybe :none
                    assert-type box $ :: 'ShownBox :number
                    assert-type some-value $ :: 'ShownMaybe :number
                    assert= |1 $ match some-value
                      (:some item) (.show item)
                      (:none) |none
                    assert= |none $ match none-value
                      (:some item) (.show item)
                      (:none) |none
              println "|✓ Data definition where-bounds passed"
          :examples $ []
          :schema $ :: :fn
            {} (:return :unit)
              :args $ []
        |unwrap-maybe $ %{} :CodeEntry (:doc "|Return payload from Maybe1 or nil")
          :code $ quote
            defn unwrap-maybe (v)
              match v
                (:none) nil
                (:some item) item
          :examples $ []
          :schema $ :: :fn
            {}
              :args $ [] (:: 'test-enum.main/Maybe1 'T)
              :generics $ [] 'T
              :return $ :: :optional 'T
      :ns $ %{} :NsEntry (:doc |)
        :code $ quote (ns test-enum.main)

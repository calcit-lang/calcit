
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |test-enum)
  :configs $ {} (:init-fn |test-enum.main/main!) (:reload-fn |test-enum.main/reload!) (:version |0.0.0)
    :modules $ []
  :entries $ {}
  :files $ {}
    |test-enum.main $ %{} :FileEntry
      :defs $ {}
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
            defn main! () $ do (println "|Testing enum runtime validation...") (test-enum-creation) (test-tag-match-validation) (test-tuple-to-enum) (test-match) (println "|All tests passed!")
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
                  :ok
                  , :ok
                (:err msg) msg
                _ :unknown
          :examples $ []
          :schema $ :: :fn
            {} (:return :dynamic)
              :args $ [] 'test-enum.main/Result0
        |test-enum-creation $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-enum-creation () $ do (println "|Testing enum tuple creation...") (; Valid tuple creation)
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
                  invalid $ %:: Result0 :invalid
                raise "|Should have failed with invalid tag"
              ; Test wrong arity $ should fail - uncomment to see error
              ; let
                  wrong-arity $ %:: Result0 :ok |extra
                raise "|Should have failed with wrong arity"
              println "|✓ Enum creation validation passed"
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
                      :ok
                      , :matched-ok
                    (:err msg) msg
                assert= :matched-ok v
              let
                  result-err $ %:: Result0 :err |some-error
                  v $ match result-err
                      :ok
                      , :matched-ok
                    (:err msg) msg
                assert= |some-error v
              ; Test exhaustive match with wildcard
              let
                  result-ok $ %:: Result0 :ok
                  v $ match result-ok
                      :ok
                      , :ok-branch
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
                      :ok
                      , :ok
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
      :ns $ %{} :NsEntry (:doc |)
        :code $ quote (ns test-enum.main)

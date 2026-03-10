
{} (:about "|file is generated - never edit directly; learn cr edit/tree workflows before changing") (:package |test-enum)
  :configs $ {} (:init-fn |test-enum.main/main!) (:reload-fn |test-enum.main/reload!) (:version |0.0.0)
    :modules $ []
  :entries $ {}
  :files $ {}
    |test-enum.main $ %{} :FileEntry
      :defs $ {}
        |Result0 $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defenum Result0 (:err :string) (:ok)
          :examples $ []
        |ResultImpl $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defimpl ResultImpl ResultTrait $ .dummy nil
          :examples $ []
        |ResultTrait $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            deftrait ResultTrait $ .dummy :fn
          :examples $ []
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! () $ do (println "|Testing enum runtime validation...") (test-enum-creation) (test-tag-match-validation) (println "|All tests passed!")
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
      :ns $ %{} :CodeEntry (:doc |) (:schema nil)
        :code $ quote (ns test-enum.main)
        :examples $ []

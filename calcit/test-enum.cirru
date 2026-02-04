
{} (:package |test-enum)
  :configs $ {} (:init-fn |test-enum.main/main!) (:reload-fn |test-enum.main/reload!)
    :modules $ []
  :files $ {}
    |test-enum.main $ %{} :FileEntry
      :defs $ {}
        |Result $ %{} :CodeEntry (:doc |)
          :code $ quote
            defenum Result
              :err :string
              :ok
        |ResultTrait $ %{} :CodeEntry (:doc |)
          :code $ quote
            deftrait ResultTrait
              :dummy (:: :fn ('T) ('T) :nil)
        |ResultImpl $ %{} :CodeEntry (:doc |)
          :code $ quote
            defrecord! ResultImpl
              :dummy nil
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! () $ do
              println "|Testing enum runtime validation..."
              test-enum-creation
              test-tag-match-validation
              println "|All tests passed!"
        |reload! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn reload! () $ println "|Reloaded"
        |test-enum-creation $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-enum-creation () $ do
              println "|Testing enum tuple creation..."
              ; Valid tuple creation
              let
                  valid-ok $ %:: Result :ok
                assert= :ok $ &tuple:nth valid-ok 0
                let
                    ok-impl $ with-traits valid-ok ResultImpl
                  assert= ResultImpl $ &list:first $ &tuple:impls ok-impl
                  assert= "|(%:: :ok (:impls ResultImpl) (:enum Result))" $ str ok-impl
              let
                  valid-err $ %:: Result :err |error-msg
                assert= :err $ &tuple:nth valid-err 0
                assert= true $ tuple? valid-err
              ; Test invalid tag (should fail - uncomment to see error)
              ; let
                  invalid $ %:: Result :invalid
                raise "|Should have failed with invalid tag"
              ; Test wrong arity (should fail - uncomment to see error)
              ; let
                  wrong-arity $ %:: Result :ok |extra
                raise "|Should have failed with wrong arity"
              println "|✓ Enum creation validation passed"
        |test-tag-match-validation $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-tag-match-validation () $ do
              println "|Testing tag-match runtime validation..."
              let
                  result $ %:: Result :ok
                  v $ tag-match result
                    (:ok) :ok
                    _ :unknown
                assert= :ok v
              println "|✓ Tag-match validation passed"
      :ns $ %{} :CodeEntry (:doc |)
        :code $ quote (ns test-enum.main)

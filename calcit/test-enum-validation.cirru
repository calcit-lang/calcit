
{} (:package |test)
  :configs $ {} (:init-fn |test.main/main!) (:reload-fn |test.main/reload!)
    :modules $ []
  :files $ {}
    |test.main $ %{} :FileEntry
      :defs $ {}
        |Result $ %{} :CodeEntry (:doc |)
          :code $ quote
            defenum Result
              :err :string
              :ok
        |ResultClass $ %{} :CodeEntry (:doc |)
          :code $ quote
            defrecord! ResultClass
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
                  valid-err $ %:: Result :err |error-msg
                assert= :err $ &tuple:nth valid-err 0
                assert= |error-msg $ &tuple:nth valid-err 1
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
                    (:ok) |success
                    (:err msg) msg
                assert= |success v
              ; Test with error variant
              let
                  result2 $ %:: Result :err |error-msg
                  v2 $ tag-match result2
                    (:ok) |success
                    (:err msg) msg
                assert= |error-msg v2
              println "|✓ Tag-match validation passed"
      :ns $ %{} :CodeEntry (:doc |)
        :code $ quote (ns test.main)

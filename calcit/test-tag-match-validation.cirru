
{} (:package |test-tag-match-validation)
  :configs $ {} (:init-fn |test-tag-match-validation.main/main!) (:reload-fn |test-tag-match-validation.main/reload!)
  :files $ {}
    |test-tag-match-validation.main $ %{} :FileEntry
      :defs $ {}
        |ResultEnum $ %{} :CodeEntry (:doc |)
          :code $ quote
            def ResultEnum $ defrecord! Result
              :err $ [] :string :string
              :ok $ []
        |ResultClass $ %{} :CodeEntry (:doc |)
          :code $ quote
            def ResultClass $ defrecord! ResultClass
              :dummy nil
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! ()
              println "|Testing tag-match enum validation..."
              test-valid-matches
              test-invalid-tag
              test-wrong-arity
              println "|All tag-match validation tests passed!"
        |reload! $ %{} :CodeEntry (:doc |)
          :code $ quote (defn reload! () nil)
        |test-valid-matches $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-valid-matches ()
              println "|  Testing valid tag-match patterns..."
              let
                  ok-tuple $ %%:: ResultClass ResultEnum :ok
                  result $ tag-match ok-tuple
                    (:ok) |ok
                    (:err msg) (str |err: msg)
                assert= |ok result
              let
                  err-tuple $ %%:: ResultClass ResultEnum :err |failed |reason
                  result $ tag-match err-tuple
                    (:ok) |ok
                    (:err msg reason) (str-spaced |err: msg reason)
                assert= (str-spaced |err: |failed |reason) result
              println "|  ✓ Valid matches work correctly"
        |test-invalid-tag $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-invalid-tag ()
              println "|  Testing invalid tag detection..."
              ; Create a valid enum tuple then corrupt its tag
              let
                  ok-tuple $ %%:: ResultClass ResultEnum :ok
                  invalid-with-enum $ &tuple:assoc ok-tuple 0 :invalid
                try
                  tag-match invalid-with-enum
                    (:invalid x) x
                    _ |default
                  fn (e)
                    if
                      includes? e "|does not have variant"
                      println "|  ✓ Invalid tag correctly detected:" e
                      raise $ str "|Unexpected error:" e
        |test-wrong-arity $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-wrong-arity ()
              println "|  Testing wrong arity detection..."
              let
                  err-tuple $ %%:: ResultClass ResultEnum :err |failed |reason
                  wrong-arity $ &tuple:assoc err-tuple 0 :ok
                println "|    Tuple:" wrong-arity
                println "|    Testing enum arity mismatch..."
                try
                  tag-match wrong-arity
                    (:ok) |ok
                    (:err msg reason) (str-spaced |err: msg reason)
                    _ |default
                  fn (e)
                    println "|    Got error:" e
                    if
                      or
                        includes? e "|expects"
                        includes? e "|payload"
                      println "|  ✓ Wrong arity (too few) detected"
                      do
                        println "|  ✗ Unexpected error type"
                        raise $ str "|Unexpected error:" e
      :ns $ %{} :CodeEntry (:doc |)
        :code $ quote (ns test-tag-match-validation.main)


{} (:about "|file is generated - never edit directly; learn cr edit/tree workflows before changing") (:package |test-tag-match-validation)
  :configs $ {} (:init-fn |test-tag-match-validation.main/main!) (:reload-fn |test-tag-match-validation.main/reload!) (:version |0.0.0)
    :modules $ []
  :entries $ {}
  :files $ {}
    |test-tag-match-validation.main $ %{} :FileEntry
      :defs $ {}
        |Result $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defenum Result (:err :string :string) (:ok)
          :examples $ []
        |ResultImpl $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defimpl ResultImpl ResultTrait $ .dummy nil
          :examples $ []
        |ResultTrait $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            deftrait ResultTrait $ .dummy :fn
          :examples $ []
        |main! $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn main! () (println "|Testing tag-match enum validation...") (test-valid-matches) (test-invalid-tag) (test-wrong-arity) (println "|All tag-match validation tests passed!")
          :examples $ []
        |reload! $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn reload! () nil
          :examples $ []
        |test-invalid-tag $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn test-invalid-tag () (println "|  Testing invalid tag detection...") (; Create a valid enum tuple then corrupt its tag)
              let
                  ok-tuple $ %:: Result :ok
                  invalid-with-enum $ &tuple:assoc ok-tuple 0 :invalid
                try
                  tag-match invalid-with-enum
                      :invalid x
                      , x
                    _ |default
                  fn (e)
                    if (includes? e "|does not have variant") (println "|  ✓ Invalid tag correctly detected:" e)
                      raise $ str "|Unexpected error:" e
          :examples $ []
        |test-valid-matches $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn test-valid-matches () (println "|  Testing valid tag-match patterns...")
              let
                  ok-tuple $ %:: Result :ok
                  result $ tag-match ok-tuple
                      :ok
                      , |ok
                    (:err msg) (str |err: msg)
                assert= |ok result
              let
                  err-tuple $ %:: Result :err |failed |reason
                  result $ tag-match err-tuple
                      :ok
                      , |ok
                    (:err msg reason) (str-spaced |err: msg reason)
                assert= (str-spaced |err: |failed |reason) result
              println "|  ✓ Valid matches work correctly"
          :examples $ []
        |test-wrong-arity $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn test-wrong-arity () (println "|  Testing wrong arity detection...")
              let
                  err-tuple $ %:: Result :err |failed |reason
                  wrong-arity $ &tuple:assoc err-tuple 0 :ok
                println "|    Tuple:" wrong-arity
                println "|    Testing enum arity mismatch..."
                try
                  tag-match wrong-arity
                      :ok
                      , |ok
                    (:err msg reason) (str-spaced |err: msg reason)
                    _ |default
                  fn (e) (println "|    Got error:" e)
                    if
                      or (includes? e |expects) (includes? e |payload)
                      println "|  ✓ Wrong arity (too few) detected"
                      do (println "|  ✗ Unexpected error type")
                        raise $ str "|Unexpected error:" e
          :examples $ []
      :ns $ %{} :CodeEntry (:doc |) (:schema nil)
        :code $ quote (ns test-tag-match-validation.main)
        :examples $ []

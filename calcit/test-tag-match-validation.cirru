
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |test-tag-match-validation) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'test-tag-match-validation.main/main!) (:mode :native) (:reload-fn 'test-tag-match-validation.main/reload!)
      :modules $ []
      :type-slots $ {}
  :files $ {}
    |test-tag-match-validation.main $ %{} :FileEntry
      :defs $ {}
        |Result $ %{} :CodeEntry (:doc |)
          :code $ quote
            defenum Result (:err 'String 'String) (:ok)
          :examples $ []
          :schema $ :: 'Dynamic
        |ResultImpl $ %{} :CodeEntry (:doc |)
          :code $ quote
            defimpl ResultImpl ResultTrait $ .dummy
              fn (_x) nil
          :examples $ []
          :schema $ :: 'Dynamic
        |ResultTrait $ %{} :CodeEntry (:doc |)
          :code $ quote
            deftrait ResultTrait $ .dummy :fn
          :examples $ []
          :schema $ :: 'Dynamic
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! () (println "|Testing tag-match enum validation...") (test-valid-matches) (test-invalid-tag) (test-wrong-arity) (println "|All tag-match validation tests passed!")
          :examples $ []
          :schema $ :: 'Dynamic
        |reload! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn reload! () nil
          :examples $ []
          :schema $ :: 'Dynamic
        |test-invalid-tag $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-invalid-tag () (println "|  Testing invalid tag detection...") (; Create a valid enum tuple then corrupt its tag)
              let
                  ok-tuple $ %:: Result :ok
                  invalid-with-enum $ &tuple:assoc ok-tuple 0 :invalid
                try
                  tag-match invalid-with-enum
                    (:invalid x) x
                    _ |default
                  fn (e)
                    if (includes? e "|does not have variant") (println "|  ✓ Invalid tag correctly detected:" e)
                      raise $ str "|Unexpected error:" e
          :examples $ []
          :schema $ :: 'Dynamic
        |test-valid-matches $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-valid-matches () (println "|  Testing valid tag-match patterns...")
              let
                  ok-tuple $ %:: Result :ok
                  result $ tag-match ok-tuple
                    (:ok) |ok
                    (:err msg) (str |err: msg)
                assert= |ok result
              let
                  err-tuple $ %:: Result :err |failed |reason
                  result $ tag-match err-tuple
                    (:ok) |ok
                    (:err msg reason) (str-spaced |err: msg reason)
                assert= (str-spaced |err: |failed |reason) result
              println "|  ✓ Valid matches work correctly"
          :examples $ []
          :schema $ :: 'Dynamic
        |test-wrong-arity $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-wrong-arity () (println "|  Testing wrong arity detection...")
              let
                  err-tuple $ %:: Result :err |failed |reason
                  wrong-arity $ &tuple:assoc err-tuple 0 :ok
                println "|    Tuple:" wrong-arity
                println "|    Testing enum arity mismatch..."
                try
                  tag-match wrong-arity
                    (:ok) |ok
                    (:err msg reason) (str-spaced |err: msg reason)
                    _ |default
                  fn (e) (println "|    Got error:" e)
                    if
                      or (includes? e |expects) (includes? e |payload)
                      println "|  ✓ Wrong arity (too few) detected"
                      do (println "|  ✗ Unexpected error type")
                        raise $ str "|Unexpected error:" e
          :examples $ []
          :schema $ :: 'Dynamic
      :ns $ %{} :NsEntry (:doc |)
        :code $ quote (ns test-tag-match-validation.main)

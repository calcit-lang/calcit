
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |test-sum-types) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'test-sum-types.main/main!) (:mode :native) (:reload-fn 'test-sum-types.main/reload!)
      :modules $ []
      :type-slots $ {}
  :files $ {}
    |test-sum-types.main $ %{} :FileEntry
      :defs $ {}
        |ActionImpl $ %{} :CodeEntry (:doc |)
          :code $ quote
            let
                ActionTrait $ deftrait ActionTrait (.describe :fn)
              defimpl ActionImpl ActionTrait $ .describe
                fn (self)
                  tag-match self
                    (:ok value) (str "|Action ok -> " value)
                    (:err message) (str "|Action err -> " message)
          :examples $ []
          :schema $ :: 'Dynamic
        |ActionResult $ %{} :CodeEntry (:doc |)
          :code $ quote
            def ActionResult $ impl-traits Result ActionImpl
          :examples $ []
          :schema $ :: 'Dynamic
        |Result $ %{} :CodeEntry (:doc |)
          :code $ quote
            defenum Result (:ok 'Number) (:err 'String)
          :examples $ []
          :schema $ :: 'Dynamic
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! () (println "|Testing sum types...")
              let
                  ok-action $ make-ok 42
                  err-action $ make-err |boom
                assert= true $ any? (&tuple:impls ok-action)
                  fn (impl)
                    option:some? $ impl-origin impl
                assert= "|(%:: :ok 42 (:enum Result))" $ str ok-action
                assert= "|Action ok -> 42" $ ok-action .describe
                assert= "|Action err -> boom" $ err-action .describe
                assert= "|handled ok 42" $ summarize ok-action
                assert= "|handled err boom" $ summarize err-action
                println "|All sum type checks passed."
              println |Done!
          :examples $ []
          :schema $ :: 'Dynamic
        |make-err $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn make-err (message) (%:: ActionResult :err message)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ [] 'Dynamic
        |make-ok $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn make-ok (value) (%:: ActionResult :ok value)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ [] 'Dynamic
        |reload! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn reload! $
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ []
        |summarize $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn summarize (action)
              tag-match action
                (:ok value) (str "|handled ok " value)
                (:err message) (str "|handled err " message)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ [] 'Dynamic
      :ns $ %{} :NsEntry (:doc |)
        :code $ quote (ns test-sum-types.main)

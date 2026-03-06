
{} (:about "|file is generated - never edit directly; learn cr edit/tree workflows before changing") (:package |test-sum-types)
  :configs $ {} (:init-fn |test-sum-types.main/main!) (:reload-fn |test-sum-types.main/reload!) (:version |0.0.0)
    :modules $ []
  :entries $ {}
  :files $ {}
    |test-sum-types.main $ %{} :FileEntry
      :defs $ {}
        |ActionImpl $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            let
                ActionTrait $ deftrait ActionTrait (.describe :fn)
              defimpl ActionImpl ActionTrait $ .describe
                fn (self)
                  tag-match self
                      :ok value
                      str "|Action ok -> " value
                    (:err message) (str "|Action err -> " message)
          :examples $ []
        |ActionResult $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            def ActionResult $ impl-traits Result ActionImpl
          :examples $ []
        |Result $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defenum Result (:ok :number) (:err :string)
          :examples $ []
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! () (println "|Testing sum types...")
              let
                  ok-action $ make-ok 42
                  err-action $ make-err |boom
                assert= true $ any? (&tuple:impls ok-action)
                  fn (impl)
                    = (&impl:origin impl) ActionTrait
                assert= "|(%:: :ok 42 (:enum Result))" $ str ok-action
                assert= "|Action ok -> 42" $ .describe ok-action
                assert= "|Action err -> boom" $ .describe err-action
                assert= "|handled ok 42" $ summarize ok-action
                assert= "|handled err boom" $ summarize err-action
                println "|All sum type checks passed."
              println |Done!
          :examples $ []
          :schema $ quote
            {} (:kind :fn)
              :args $ []
              :return :dynamic
        |make-err $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn make-err (message) (%:: ActionResult :err message)
          :examples $ []
          :schema $ quote
            {} (:kind :fn)
              :args $ [] (:: 'message :dynamic)
              :return :dynamic
        |make-ok $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn make-ok (value) (%:: ActionResult :ok value)
          :examples $ []
          :schema $ quote
            {} (:kind :fn)
              :args $ [] (:: 'value :dynamic)
              :return :dynamic
        |reload! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn reload! () nil
          :examples $ []
          :schema $ quote
            {} (:kind :fn)
              :args $ []
              :return :dynamic
        |summarize $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn summarize (action)
              tag-match action
                  :ok value
                  str "|handled ok " value
                (:err message) (str "|handled err " message)
          :examples $ []
          :schema $ quote
            {} (:kind :fn)
              :args $ [] (:: 'action :dynamic)
              :return :dynamic
      :ns $ %{} :CodeEntry (:doc |) (:schema nil)
        :code $ quote (ns test-sum-types.main)
        :examples $ []

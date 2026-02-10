
{} (:package |test-sum-types)
  :configs $ {} (:init-fn |test-sum-types.main/main!) (:reload-fn |test-sum-types.main/reload!)
  :files $ {}
    |test-sum-types.main $ %{} :FileEntry
      :defs $ {}
        |Result $ %{} :CodeEntry (:doc |)
          :code $ quote
            defenum Result
              :ok :number
              :err :string
        |ActionImpl $ %{} :CodeEntry (:doc |)
          :code $ quote
            let
                ActionTrait $ deftrait ActionTrait
                  :describe :fn
              defimpl ActionImpl ActionTrait
                :describe $ fn (self)
                  tag-match self
                    (:ok value) (str "|Action ok -> " value)
                    (:err message) (str "|Action err -> " message)
        |make-ok $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn make-ok (value)
              impl-traits (%:: Result :ok value) ActionImpl
        |make-err $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn make-err (message)
              impl-traits (%:: Result :err message) ActionImpl
        |summarize $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn summarize (action)
              tag-match action
                (:ok value) (str "|handled ok " value)
                (:err message) (str "|handled err " message)
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! ()
              println "|Testing sum types..."
              let
                  ok-action $ make-ok 42
                  err-action $ make-err "|boom"
                assert= true $ any? (&tuple:impls ok-action)
                  fn (impl) $ = (&impl:origin impl) ActionTrait
                assert= "|(%:: :ok 42 (:enum Result))" $ str ok-action
                assert= "|Action ok -> 42" (.describe ok-action)
                assert= "|Action err -> boom" (.describe err-action)
                assert= "|handled ok 42" $ summarize ok-action
                assert= "|handled err boom" $ summarize err-action
                println "|All sum type checks passed."
              println "|Done!"
        |reload! $ %{} :CodeEntry (:doc |)
          :code $ quote (defn reload! () nil)
      :ns $ %{} :CodeEntry (:doc |)
        :code $ quote (ns test-sum-types.main)

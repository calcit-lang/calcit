
{} (:package |test-sum-types)
  :configs $ {} (:init-fn |test-sum-types.main/main!) (:reload-fn |test-sum-types.main/reload!)
  :files $ {}
    |test-sum-types.main $ %{} :FileEntry
      :defs $ {}
        |ResultEnum $ %{} :CodeEntry (:doc |)
          :code $ quote
            def ResultEnum $ defrecord! Result
              :ok $ [] :number
              :err $ [] :string
        |ActionClass $ %{} :CodeEntry (:doc |)
          :code $ quote
            def ActionClass $ defrecord! ActionClass
              :describe $ fn (self)
                tag-match self
                  (:ok value) (str "|Action ok -> " value)
                  (:err message) (str "|Action err -> " message)
        |make-ok $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn make-ok (value)
              %%:: ActionClass ResultEnum :ok value
        |make-err $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn make-err (message)
              %%:: ActionClass ResultEnum :err message
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
                assert= ActionClass $ &tuple:class ok-action
                assert= "|(%%:: :ok 42 (:class ActionClass) (:enum Result))" $ str ok-action
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

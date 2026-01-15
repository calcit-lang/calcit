
{} (:package |test-invalid-tag)
  :configs $ {} (:init-fn |test-invalid-tag.main/main!) (:reload-fn |test-invalid-tag.main/reload!)
  :files $ {}
    |test-invalid-tag.main $ %{} :FileEntry
      :defs $ {}
        |ResultEnum $ %{} :CodeEntry (:doc |)
          :code $ quote
            def ResultEnum $ defrecord! Result
              :err $ [] :string
              :ok $ []
        |ResultClass $ %{} :CodeEntry (:doc |)
          :code $ quote
            def ResultClass $ defrecord! ResultClass
              :dummy nil
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! ()
              println "|Testing %%:: call"
              println "|ResultEnum:" ResultEnum
              println "|ResultClass:" ResultClass
              ; Direct call to %%:: to see if function is invoked
              println "|Calling %%:: ..."
              let
                  result $ %%:: ResultClass ResultEnum :invalid
                println "|Should not reach here:" result
        |reload! $ %{} :CodeEntry (:doc |)
          :code $ quote (defn reload! () nil)
      :ns $ %{} :CodeEntry (:doc |)
        :code $ quote (ns test-invalid-tag.main)

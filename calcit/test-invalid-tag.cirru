
{} (:package |test-invalid-tag)
  :configs $ {} (:init-fn |test-invalid-tag.main/main!) (:reload-fn |test-invalid-tag.main/reload!)
  :files $ {}
    |test-invalid-tag.main $ %{} :FileEntry
      :defs $ {}
        |Result $ %{} :CodeEntry (:doc |)
          :code $ quote
            defenum Result
              :err :string
              :ok
        |ResultImpl $ %{} :CodeEntry (:doc |)
          :code $ quote
            defrecord! ResultImpl
              :dummy nil
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! ()
              println "|Testing %:: call"
              println "|Result:" Result
              println "|ResultImpl:" ResultImpl
                ; Direct call to %:: to see if function is invoked
                println "|Calling %:: ..."
              let
                  result $ %:: Result :invalid
                println "|Should not reach here:" result
        |reload! $ %{} :CodeEntry (:doc |)
          :code $ quote (defn reload! () nil)
      :ns $ %{} :CodeEntry (:doc |)
        :code $ quote (ns test-invalid-tag.main)

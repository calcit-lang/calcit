
{} (:about "|file is generated - never edit directly; learn cr edit/tree workflows before changing") (:package |test-invalid-tag)
  :configs $ {} (:init-fn |test-invalid-tag.main/main!) (:reload-fn |test-invalid-tag.main/reload!) (:version |0.0.0)
    :modules $ []
  :entries $ {}
  :files $ {}
    |test-invalid-tag.main $ %{} :FileEntry
      :defs $ {}
        |Result $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defenum Result (:err :string) (:ok)
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
            defn main! () (println "|Testing %:: call") (println |Result: Result)
              println |ResultImpl: ResultImpl (; Direct call to %:: to see if function is invoked) (println "|Calling %:: ...")
              let
                  result $ %:: Result :invalid
                println "|Should not reach here:" result
          :examples $ []
        |reload! $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn reload! () nil
          :examples $ []
      :ns $ %{} :CodeEntry (:doc |) (:schema nil)
        :code $ quote (ns test-invalid-tag.main)
        :examples $ []

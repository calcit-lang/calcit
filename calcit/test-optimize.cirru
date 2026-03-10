
{} (:about "|file is generated - never edit directly; learn cr edit/tree workflows before changing") (:package |test-optimize)
  :configs $ {} (:init-fn |test-optimize.main/main!) (:reload-fn |test-optimize.main/reload!) (:version |0.0.0)
    :modules $ []
  :entries $ {}
  :files $ {}
    |test-optimize.main $ %{} :FileEntry
      :defs $ {}
        |LocalPerson0 $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defstruct LocalPerson0 $ :name :string
          :examples $ []
        |Person $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            def Person $ impl-traits Person0 ShowImpl
          :examples $ []
        |Person0 $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defstruct Person0 $ :name :string
          :examples $ []
        |ShowImpl $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defimpl ShowImpl ShowTrait $ .show
              fn (self)
                str "|Person: " $ &record:get self :name
          :examples $ []
        |ShowTrait $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            deftrait ShowTrait $ .show :fn
          :examples $ []
        |main! $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn main! () $ let
                p $ %{} Person (:name |Jim)
              println "|--- direct call ---"
              println $ .show p
              let
                  p2 p
                assert-traits p2 ShowTrait
                println "|--- assert-traits ShowTrait ---"
                println $ .show p2
              let
                  p3 p
                assert-type p3 Person
                println "|--- assert-type Person ---"
                println $ .show p3
              let
                  p4 p
                assert-type p4 Person
                assert-traits p4 ShowTrait
                println "|--- assert-type Person + assert-traits ShowTrait ---"
                println $ .show p4
              let
                LocalPerson $ impl-traits LocalPerson0 ShowImpl
                  lp $ %{} LocalPerson (:name |Local)
                println "|--- local struct (runtime impl) ---"
                assert-traits lp ShowTrait
                println $ .show lp
          :examples $ []
      :ns $ %{} :NsEntry (:doc |)
        :code $ quote
          ns test-optimize.main $ :require

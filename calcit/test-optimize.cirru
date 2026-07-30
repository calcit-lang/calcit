
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |test-optimize) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:description |) (:init-fn |test-optimize.main/main!) (:mode :native) (:reload-fn |test-optimize.main/reload!)
      :modules $ []
      :type-slots $ {}
  :files $ {}
    |test-optimize.main $ %{} :FileEntry
      :defs $ {}
        |LocalPerson0 $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defstruct LocalPerson0 $ :name :string
          :examples $ []
        |Person $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            def Person $ impl-traits Person0 ShowImpl
          :examples $ []
        |Person0 $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defstruct Person0 $ :name :string
          :examples $ []
        |ShowImpl $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defimpl ShowImpl ShowTrait $ .show
              fn (self)
                str "|Person: " $ &record:get self :name
          :examples $ []
        |ShowTrait $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            deftrait ShowTrait $ .show :fn
          :examples $ []
        |main! $ %{} :CodeEntry (:doc |) (:schema :dynamic)
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

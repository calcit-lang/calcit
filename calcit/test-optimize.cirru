
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |test-optimize) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'test-optimize.main/main!) (:mode :native) (:reload-fn 'test-optimize.main/reload!)
      :modules $ []
      :type-slots $ {}
  :files $ {}
    |test-optimize.main $ %{} :FileEntry
      :defs $ {}
        |LocalPerson0 $ %{} :CodeEntry (:doc |)
          :code $ quote
            defstruct LocalPerson0 $ :name 'String
          :examples $ []
          :schema $ :: 'Dynamic
        |Person $ %{} :CodeEntry (:doc |)
          :code $ quote
            def Person $ impl-traits Person0 ShowImpl
          :examples $ []
          :schema $ :: 'Dynamic
        |Person0 $ %{} :CodeEntry (:doc |)
          :code $ quote
            defstruct Person0 $ :name 'String
          :examples $ []
          :schema $ :: 'Dynamic
        |ShowImpl $ %{} :CodeEntry (:doc |)
          :code $ quote
            defimpl ShowImpl ShowTrait $ .show
              fn (self)
                str "|Person: " $ &record:get self :name
          :examples $ []
          :schema $ :: 'Dynamic
        |ShowTrait $ %{} :CodeEntry (:doc |)
          :code $ quote
            deftrait ShowTrait $ .show :fn
          :examples $ []
          :schema $ :: 'Dynamic
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! () $ let
                p $ %{} Person (:name |Jim)
              println "|--- direct call ---"
              println $ p .show
              let
                  p2 p
                assert-traits p2 ShowTrait
                println "|--- assert-traits ShowTrait ---"
                println $ p2 .show
              let
                  p3 p
                assert-type p3 Person
                println "|--- assert-type Person ---"
                println $ p3 .show
              let
                  p4 p
                assert-type p4 Person
                assert-traits p4 ShowTrait
                println "|--- assert-type Person + assert-traits ShowTrait ---"
                println $ p4 .show
              let
                LocalPerson $ impl-traits LocalPerson0 ShowImpl
                  lp $ %{} LocalPerson (:name |Local)
                println "|--- local struct (runtime impl) ---"
                assert-traits lp ShowTrait
                println $ lp .show
          :examples $ []
          :schema $ :: 'Dynamic
      :ns $ %{} :NsEntry (:doc |)
        :code $ quote
          ns test-optimize.main $ :require

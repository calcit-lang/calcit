{} (:package |test-doc-smoke)
  :configs $ {} (:init-fn |test-doc-smoke.main/main!) (:reload-fn |test-doc-smoke.main/reload!)
  :files $ {}
    |test-doc-smoke.main $ %{} :FileEntry
      :defs $ {}
        |DocTrait $ %{} :CodeEntry (:doc "|Doc smoke trait")
          :code $ quote
            deftrait DocTrait
              :label :fn
          :examples $ []
        |DocTraitImpl $ %{} :CodeEntry (:doc "|Doc smoke impl")
          :code $ quote
            defimpl DocTraitImpl DocTrait
              :label $ fn (x)
                str-spaced |doc (:name x)
          :examples $ []
        |DocPerson0 $ %{} :CodeEntry (:doc "|Doc smoke struct")
          :code $ quote
            defstruct DocPerson
              :name :string
          :examples $ []
        |DocEnum0 $ %{} :CodeEntry (:doc "|Doc smoke enum")
          :code $ quote
            defenum DocEnum
              :ok :string
          :examples $ []
        |main! $ %{} :CodeEntry (:doc "|Run docs smoke cases")
          :code $ quote
            defn main! ()
              println "|Testing doc smoke cases..."
              test-defimpl-order
              test-assert-traits-local
              test-impl-traits-struct-enum-only
              println "|Doc smoke cases passed"
          :examples $ []
        |reload! $ %{} :CodeEntry (:doc "|Reload handler")
          :code $ quote
            defn reload! () $ :: :unit
          :examples $ []
        |test-defimpl-order $ %{} :CodeEntry (:doc "|defimpl arg order smoke")
          :code $ quote
            defn test-defimpl-order ()
              assert= DocTrait $ &impl:origin DocTraitImpl
          :examples $ []
        |test-assert-traits-local $ %{} :CodeEntry (:doc "|assert-traits local first arg smoke")
          :code $ quote
            defn test-assert-traits-local ()
              let
                  DocPerson $ impl-traits DocPerson0 DocTraitImpl
                  p $ %{} DocPerson (:name |Alice)
                assert= p $ assert-traits p DocTrait
                assert= "|doc Alice" $ .label p
          :examples $ []
        |test-impl-traits-struct-enum-only $ %{} :CodeEntry (:doc "|impl-traits only accepts struct/enum definitions")
          :code $ quote
            defn test-impl-traits-struct-enum-only ()
              let
                  DocPerson $ impl-traits DocPerson0 DocTraitImpl
                  DocEnum $ impl-traits DocEnum0 DocTraitImpl
                assert= true $ struct? DocPerson
                assert= true $ enum? DocEnum
              let
                  msg $ try
                    do
                      impl-traits (%:: DocEnum0 :ok |done) DocTraitImpl
                      , |NO_ERROR
                    fn (e) $ str e
                assert= false $ &= msg |NO_ERROR
                inside-eval:
                  assert= true $ includes? msg |Expected:
                  assert= true $ includes? msg |Actual:
                  assert= true $ includes? msg |Fix:
          :examples $ []
      :ns $ %{} :CodeEntry (:doc |)
        :code $ quote
          ns test-doc-smoke.main $ :require
            util.core :refer $ inside-eval:
        :examples $ []
